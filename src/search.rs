use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, TantivyDocument, Value, STORED, STRING, TEXT};
use tantivy::snippet::SnippetGenerator;
use tantivy::{doc, Index, IndexWriter};

use crate::vault::BRAIN_DIR;

const INDEX_HEAP_BYTES: usize = 50_000_000;

pub struct SearchHit {
    pub title: String,
    pub snippet: String,
}

fn build_schema() -> (Schema, tantivy::schema::Field, tantivy::schema::Field) {
    let mut builder = Schema::builder();
    let title = builder.add_text_field("title", STRING | STORED);
    let body = builder.add_text_field("body", TEXT | STORED);
    (builder.build(), title, body)
}

fn index_dir(vault: &Path) -> std::path::PathBuf {
    vault.join(BRAIN_DIR).join("tantivy")
}

/// Rebuild the full-text index from scratch for the given `(title, body)` notes.
pub fn rebuild(vault: &Path, notes: &[(String, String)]) -> Result<()> {
    let dir = index_dir(vault);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .with_context(|| format!("clearing index dir {}", dir.display()))?;
    }
    fs::create_dir_all(&dir).with_context(|| format!("creating index dir {}", dir.display()))?;

    let (schema, title_field, body_field) = build_schema();
    let index = Index::create_in_dir(&dir, schema).context("creating tantivy index")?;
    let mut writer: IndexWriter = index
        .writer(INDEX_HEAP_BYTES)
        .context("creating index writer")?;

    for (title, body) in notes {
        writer.add_document(doc!(
            title_field => title.as_str(),
            body_field => body.as_str(),
        ))?;
    }
    writer.commit().context("committing index")?;
    Ok(())
}

/// Run a full-text query against the vault's index, returning matches with a highlighted snippet.
pub fn query(vault: &Path, text: &str) -> Result<Vec<SearchHit>> {
    let dir = index_dir(vault);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let index = Index::open_in_dir(&dir).context("opening tantivy index")?;
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
}
