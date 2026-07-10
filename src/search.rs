use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    IndexRecordOption, Schema, TantivyDocument, TextFieldIndexing, TextOptions, Value, STORED,
    STRING,
};
use tantivy::snippet::SnippetGenerator;
use tantivy::tokenizer::{LowerCaser, RemoveLongFilter, TextAnalyzer};
use tantivy::{doc, Index, IndexWriter, Term};

use crate::thai::ThaiTokenizer;
use crate::vault::BRAIN_DIR;

const INDEX_HEAP_BYTES: usize = 50_000_000;
/// Mixed Thai/non-Thai tokenizer; must be registered on every opened index.
const THAI_TOKENIZER_NAME: &str = "thai_mixed";

fn register_tokenizers(index: &Index) {
    index.tokenizers().register(
        THAI_TOKENIZER_NAME,
        TextAnalyzer::builder(ThaiTokenizer)
            .filter(RemoveLongFilter::limit(100))
            .filter(LowerCaser)
            .build(),
    );
}

pub struct SearchHit {
    pub title: String,
    pub snippet: String,
}

fn build_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("title", STRING | STORED);
    let body_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(THAI_TOKENIZER_NAME)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    builder.add_text_field("body", body_options);
    builder.build()
}

fn index_dir(vault: &Path) -> std::path::PathBuf {
    vault.join(BRAIN_DIR).join("tantivy")
}

/// Open the vault's persistent index, creating an empty one on first use.
fn open_or_create(vault: &Path) -> Result<Index> {
    let dir = index_dir(vault);
    let index = if dir.exists() {
        Index::open_in_dir(&dir).context("opening tantivy index")?
    } else {
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating index dir {}", dir.display()))?;
        Index::create_in_dir(&dir, build_schema()).context("creating tantivy index")?
    };
    register_tokenizers(&index);
    Ok(index)
}

/// Apply an incremental batch: upsert changed notes and drop removed ones,
/// in a single commit. The title field is raw-indexed, so `delete_term` on it
/// removes exactly one note's previous document.
pub fn apply(vault: &Path, upserts: &[(String, String)], removals: &[String]) -> Result<()> {
    let index = open_or_create(vault)?;
    let schema = index.schema();
    let title_field = schema.get_field("title")?;
    let body_field = schema.get_field("body")?;

    let mut writer: IndexWriter = index
        .writer(INDEX_HEAP_BYTES)
        .context("creating index writer")?;

    for title in removals {
        writer.delete_term(Term::from_field_text(title_field, title));
    }
    for (title, body) in upserts {
        writer.delete_term(Term::from_field_text(title_field, title));
        writer.add_document(doc!(
            title_field => title.as_str(),
            body_field => body.as_str(),
        ))?;
    }
    writer.commit().context("committing index")?;
    Ok(())
}

/// Rebuild the full-text index from scratch for the given `(title, body)` notes.
pub fn rebuild(vault: &Path, notes: &[(String, String)]) -> Result<()> {
    let dir = index_dir(vault);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .with_context(|| format!("clearing index dir {}", dir.display()))?;
    }
    apply(vault, notes, &[])
}

/// Run a full-text query against the vault's index, returning matches with a highlighted snippet.
pub fn query(vault: &Path, text: &str) -> Result<Vec<SearchHit>> {
    let dir = index_dir(vault);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let index = Index::open_in_dir(&dir).context("opening tantivy index")?;
    register_tokenizers(&index);
    let schema = index.schema();
    let title_field = schema
        .get_field("title")
        .context("schema missing title field")?;
    let body_field = schema
        .get_field("body")
        .context("schema missing body field")?;

    let reader = index.reader().context("creating index reader")?;
    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![title_field, body_field]);
    let parsed_query = query_parser
        .parse_query(text)
        .context("parsing search query")?;

    let snippet_generator = SnippetGenerator::create(&searcher, &*parsed_query, body_field)
        .context("creating snippet generator")?;

    let top_docs = searcher.search(&*parsed_query, &TopDocs::with_limit(20).order_by_score())?;
    let mut hits = Vec::new();
    for (_score, doc_address) in top_docs {
        let retrieved: TantivyDocument = searcher.doc(doc_address)?;
        let title = retrieved
            .get_first(title_field)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let snippet = snippet_generator.snippet_from_doc(&retrieved);
        let snippet_text = if snippet.is_empty() {
            retrieved
                .get_first(body_field)
                .and_then(|v| v.as_str())
                .map(|s| s.chars().take(120).collect::<String>())
                .unwrap_or_default()
        } else {
            snippet.to_html()
        };
        hits.push(SearchHit {
            title,
            snippet: snippet_text,
        });
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_then_query_finds_match() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[
                (
                    "Rust".to_string(),
                    "Rust is a systems programming language".to_string(),
                ),
                (
                    "Cooking".to_string(),
                    "How to bake bread at home".to_string(),
                ),
            ],
        )
        .unwrap();

        let hits = query(dir.path(), "systems programming").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Rust");
    }

    #[test]
    fn query_with_no_index_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let hits = query(dir.path(), "anything").unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn rebuild_replaces_previous_documents() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[("A".to_string(), "alpha content".to_string())],
        )
        .unwrap();
        assert_eq!(query(dir.path(), "alpha").unwrap().len(), 1);

        rebuild(dir.path(), &[("B".to_string(), "beta content".to_string())]).unwrap();
        assert!(query(dir.path(), "alpha").unwrap().is_empty());
        assert_eq!(query(dir.path(), "beta").unwrap().len(), 1);
    }

    #[test]
    fn apply_upserts_single_note_without_touching_others() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[
                ("A".to_string(), "alpha content".to_string()),
                ("B".to_string(), "beta content".to_string()),
            ],
        )
        .unwrap();

        apply(
            dir.path(),
            &[("A".to_string(), "gamma content".to_string())],
            &[],
        )
        .unwrap();

        assert!(query(dir.path(), "alpha").unwrap().is_empty());
        assert_eq!(query(dir.path(), "gamma").unwrap().len(), 1);
        assert_eq!(query(dir.path(), "beta").unwrap().len(), 1);
    }

    #[test]
    fn apply_removes_deleted_note() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[
                ("A".to_string(), "alpha content".to_string()),
                ("B".to_string(), "beta content".to_string()),
            ],
        )
        .unwrap();

        apply(dir.path(), &[], &["A".to_string()]).unwrap();

        assert!(query(dir.path(), "alpha").unwrap().is_empty());
        assert_eq!(query(dir.path(), "beta").unwrap().len(), 1);
    }
}
