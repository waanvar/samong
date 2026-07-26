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

/// A note as handed to the index: identity, display name, and content.
pub struct IndexedNote {
    /// Vault-relative path — the document's identity in the index.
    pub key: String,
    pub title: String,
    pub body: String,
}

pub struct SearchHit {
    /// Vault-relative path of the matching note.
    pub key: String,
    pub title: String,
    pub snippet: String,
}

fn build_schema() -> Schema {
    let mut builder = Schema::builder();
    // Raw-indexed identity: `delete_term` on it replaces exactly one document.
    // Titles cannot serve here — a repo can hold many `README.md` files, and
    // keying on the title made them overwrite each other on incremental runs
    // while piling up as duplicates on a full rebuild.
    builder.add_text_field("path", STRING | STORED);
    // Titles are worth searching, and tokenized the same way bodies are so a
    // Thai title matches mid-word too.
    let title_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(THAI_TOKENIZER_NAME)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    builder.add_text_field("title", title_options);
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

/// Apply an incremental batch: upsert changed notes and drop removed ones
/// (by key), in a single commit. The path field is raw-indexed, so
/// `delete_term` on it removes exactly one note's previous document.
pub fn apply(vault: &Path, upserts: &[IndexedNote], removals: &[String]) -> Result<()> {
    let index = open_or_create(vault)?;
    let schema = index.schema();
    let path_field = schema.get_field("path")?;
    let title_field = schema.get_field("title")?;
    let body_field = schema.get_field("body")?;

    let mut writer: IndexWriter = index
        .writer(INDEX_HEAP_BYTES)
        .context("creating index writer")?;

    for key in removals {
        writer.delete_term(Term::from_field_text(path_field, key));
    }
    for note in upserts {
        writer.delete_term(Term::from_field_text(path_field, &note.key));
        writer.add_document(doc!(
            path_field => note.key.as_str(),
            title_field => note.title.as_str(),
            body_field => note.body.as_str(),
        ))?;
    }
    writer.commit().context("committing index")?;
    Ok(())
}

/// Rebuild the full-text index from scratch for the given notes.
pub fn rebuild(vault: &Path, notes: &[IndexedNote]) -> Result<()> {
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
    let path_field = schema
        .get_field("path")
        .context("schema missing path field")?;
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
        let stored = |field| {
            retrieved
                .get_first(field)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let key = stored(path_field);
        let title = stored(title_field);
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
            key,
            title,
            snippet: snippet_text,
        });
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A note whose key is `<title>.md` at the vault root.
    fn note(title: &str, body: &str) -> IndexedNote {
        IndexedNote {
            key: format!("{title}.md"),
            title: title.to_string(),
            body: body.to_string(),
        }
    }

    fn at(key: &str, title: &str, body: &str) -> IndexedNote {
        IndexedNote {
            key: key.to_string(),
            title: title.to_string(),
            body: body.to_string(),
        }
    }

    #[test]
    fn rebuild_then_query_finds_match() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[
                note("Rust", "Rust is a systems programming language"),
                note("Cooking", "How to bake bread at home"),
            ],
        )
        .unwrap();

        let hits = query(dir.path(), "systems programming").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Rust");
        assert_eq!(hits[0].key, "Rust.md");
    }

    #[test]
    fn query_with_no_index_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let hits = query(dir.path(), "anything").unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn titles_are_searchable() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(dir.path(), &[note("Deployment Runbook", "body text")]).unwrap();
        assert_eq!(query(dir.path(), "runbook").unwrap().len(), 1);
    }

    #[test]
    fn rebuild_replaces_previous_documents() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(dir.path(), &[note("A", "alpha content")]).unwrap();
        assert_eq!(query(dir.path(), "alpha").unwrap().len(), 1);

        rebuild(dir.path(), &[note("B", "beta content")]).unwrap();
        assert!(query(dir.path(), "alpha").unwrap().is_empty());
        assert_eq!(query(dir.path(), "beta").unwrap().len(), 1);
    }

    #[test]
    fn apply_upserts_single_note_without_touching_others() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[note("A", "alpha content"), note("B", "beta content")],
        )
        .unwrap();

        apply(dir.path(), &[note("A", "gamma content")], &[]).unwrap();

        assert!(query(dir.path(), "alpha").unwrap().is_empty());
        assert_eq!(query(dir.path(), "gamma").unwrap().len(), 1);
        assert_eq!(query(dir.path(), "beta").unwrap().len(), 1);
    }

    #[test]
    fn apply_removes_deleted_note() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[note("A", "alpha content"), note("B", "beta content")],
        )
        .unwrap();

        apply(dir.path(), &[], &["A.md".to_string()]).unwrap();

        assert!(query(dir.path(), "alpha").unwrap().is_empty());
        assert_eq!(query(dir.path(), "beta").unwrap().len(), 1);
    }

    /// Notes sharing a title are separate documents: updating one must not
    /// silently delete the others, which is what keying on title used to do.
    #[test]
    fn same_title_in_different_directories_are_separate_documents() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[
                at("README.md", "README", "root readme alpha"),
                at("docs/README.md", "README", "docs readme beta"),
            ],
        )
        .unwrap();

        assert_eq!(query(dir.path(), "readme").unwrap().len(), 2);

        // Rewriting one leaves the other intact.
        apply(
            dir.path(),
            &[at("docs/README.md", "README", "docs readme gamma")],
            &[],
        )
        .unwrap();

        assert_eq!(query(dir.path(), "alpha").unwrap().len(), 1);
        assert!(query(dir.path(), "beta").unwrap().is_empty());
        let gamma = query(dir.path(), "gamma").unwrap();
        assert_eq!(gamma.len(), 1);
        assert_eq!(gamma[0].key, "docs/README.md");

        // And removing one by key leaves the other.
        apply(dir.path(), &[], &["docs/README.md".to_string()]).unwrap();
        let left = query(dir.path(), "readme").unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].key, "README.md");
    }
}
