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

/// Hits returned when the caller does not ask for a specific number.
pub const DEFAULT_LIMIT: usize = 20;
/// Ceiling on what any caller may request, so a single query cannot return the
/// whole vault.
pub const MAX_LIMIT: usize = 100;
/// Characters of surrounding context shown per hit.
pub const DEFAULT_SNIPPET_CHARS: usize = 150;

/// How much a well-connected note may be boosted, at most: +25%.
///
/// Deliberately small. A note many others link to is probably the one you want,
/// but that is a hint, not an argument — a note that barely matches the words
/// must never outrank one that matches them well just because it is popular.
/// At this weight connectedness only reorders results that were already close,
/// which is exactly the job.
const DEGREE_WEIGHT: f32 = 0.25;
/// Degree at which the boost is already at its maximum. Shared with the graph
/// view, which stops growing node radius at the same count, so a node that looks
/// like a hub ranks like one.
const DEGREE_SATURATION: f32 = 12.0;
/// Candidates fetched per requested hit before re-ranking.
///
/// Re-ranking only the hits you were going to return can reorder them but can
/// never promote a well-connected note that BM25 put just outside the cut, which
/// is the case worth catching.
const RERANK_POOL_FACTOR: usize = 3;
/// Ceiling on the candidate pool — the same `MAX_LIMIT` that caps callers, so
/// there is exactly one number for "most documents this code will ever fetch"
/// and no internal back door around it. Each candidate costs a stored-document
/// fetch and a snippet, so it is a real budget.
///
/// The consequence is that a caller asking for the maximum gets no overfetch and
/// so no promotion from outside the cut, only re-ordering within it. Asking for
/// a hundred hits is already asking to see everything.
const RERANK_POOL_MAX: usize = MAX_LIMIT;

/// How much a search should return.
///
/// Worth controlling because the caller is often an AI agent, where every hit
/// is context it pays for on every subsequent turn: 20 hits of Thai text is a
/// few thousand tokens, and Thai costs noticeably more tokens per character
/// than English. A human scanning a terminal wants a long list; an agent
/// looking for one note does not.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub limit: usize,
    pub snippet_chars: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            snippet_chars: DEFAULT_SNIPPET_CHARS,
        }
    }
}

impl SearchOptions {
    /// Return at most `limit` hits, clamped into `1..=MAX_LIMIT`.
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit: limit.clamp(1, MAX_LIMIT),
            ..Self::default()
        }
    }

    fn limit(&self) -> usize {
        self.limit.clamp(1, MAX_LIMIT)
    }

    fn snippet_chars(&self) -> usize {
        self.snippet_chars.max(20)
    }
}

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
    /// Ranking score. Kept so callers merging hits from several vaults can rank
    /// them together instead of just concatenating per-vault lists.
    ///
    /// From [`query_with`] this is BM25 relevance. From [`query_ranked`] it also
    /// carries the connectedness boost, which is why the cross-vault merge in
    /// `mcp` sorts on it and gets a consistent order.
    pub score: f32,
    /// Set when the note belongs to an installed vault rather than to the
    /// reader. Filled in by [`crate::ops::search_vault`] and not here: this
    /// module knows about an index, and whose notes are in it is a question
    /// about scope.
    pub source: Option<crate::provenance::Source>,
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

/// Run a full-text query against the vault's index with default limits.
pub fn query(vault: &Path, text: &str) -> Result<Vec<SearchHit>> {
    query_with(vault, text, &SearchOptions::default())
}

/// The multiplier a note's connectedness earns it: `1.0` when nothing links to
/// it, rising to `1 + DEGREE_WEIGHT` and no further.
///
/// Logarithmic, so the first few links matter most and a note with fifty does not
/// bury one with five. Saturating, so a hub cannot dominate every query.
fn degree_boost(degree: usize) -> f32 {
    if degree == 0 {
        return 1.0;
    }
    let scaled = (1.0 + degree as f32).ln() / (1.0 + DEGREE_SATURATION).ln();
    1.0 + DEGREE_WEIGHT * scaled.min(1.0)
}

/// Fetch specific notes from the index by key, as hits.
///
/// For candidates that another ranking found and full-text search did not — a
/// semantic match on a note that shares no words with the query. Without this,
/// hybrid search could only reorder what BM25 already returned, which is useless
/// in exactly the case semantic search exists for: not remembering the words you
/// wrote.
///
/// Snippets are the opening of the note, because no query term matched anything to
/// highlight. Keys absent from the index are skipped rather than reported: they
/// are notes the text index does not know about, and returning one would hand back
/// a result that cannot be opened.
pub fn hits_for_keys(
    vault: &Path,
    keys: &[String],
    snippet_chars: usize,
) -> Result<Vec<SearchHit>> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let dir = index_dir(vault);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let index = Index::open_in_dir(&dir).context("opening tantivy index")?;
    register_tokenizers(&index);
    let schema = index.schema();
    let path_field = schema.get_field("path")?;
    let title_field = schema.get_field("title")?;
    let body_field = schema.get_field("body")?;
    let searcher = index.reader().context("creating index reader")?.searcher();

    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        let term = Term::from_field_text(path_field, key);
        let query = tantivy::query::TermQuery::new(term, IndexRecordOption::Basic);
        let found = searcher.search(&query, &TopDocs::with_limit(1).order_by_score())?;
        let Some((_, address)) = found.first() else {
            continue;
        };
        let retrieved: TantivyDocument = searcher.doc(*address)?;
        let stored = |field| {
            retrieved
                .get_first(field)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };
        out.push(SearchHit {
            key: key.clone(),
            title: stored(title_field),
            snippet: stored(body_field)
                .chars()
                .take(snippet_chars.max(20))
                .collect(),
            // Filled in by whoever fuses the rankings; on its own this hit has no
            // relevance score, because relevance is not why it is here.
            score: 0.0,
            source: None,
        });
    }
    Ok(out)
}

/// Search, then rank by relevance *and* connectedness.
///
/// Fetches a larger pool than asked for, multiplies each BM25 score by the
/// note's [`degree_boost`], re-sorts and truncates. `degrees` comes from
/// [`crate::graph::Graph::degrees`]; a key that is absent scores as unconnected.
///
/// `SearchHit::score` carries the boosted value, because callers merging several
/// vaults sort on it and must compare like with like.
pub fn query_ranked(
    vault: &Path,
    text: &str,
    options: &SearchOptions,
    degrees: &std::collections::HashMap<String, usize>,
) -> Result<Vec<SearchHit>> {
    let wanted = options.limit();
    let pool = SearchOptions {
        limit: (wanted * RERANK_POOL_FACTOR).min(RERANK_POOL_MAX),
        snippet_chars: options.snippet_chars,
    };
    let mut hits = query_with(vault, text, &pool)?;
    for hit in &mut hits {
        hit.score *= degree_boost(degrees.get(&hit.key).copied().unwrap_or(0));
    }
    // Ties broken by key so the order is stable across runs and machines.
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.key.cmp(&b.key))
    });
    hits.truncate(wanted);
    Ok(hits)
}

/// Run a full-text query, returning matches with a highlighted snippet.
pub fn query_with(vault: &Path, text: &str, options: &SearchOptions) -> Result<Vec<SearchHit>> {
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

    let mut snippet_generator = SnippetGenerator::create(&searcher, &*parsed_query, body_field)
        .context("creating snippet generator")?;
    snippet_generator.set_max_num_chars(options.snippet_chars());

    let top_docs = searcher.search(
        &*parsed_query,
        &TopDocs::with_limit(options.limit()).order_by_score(),
    )?;
    let mut hits = Vec::new();
    for (score, doc_address) in top_docs {
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
            // No term matched the body (a title-only hit): show the opening
            // instead, trimmed to the same budget.
            retrieved
                .get_first(body_field)
                .and_then(|v| v.as_str())
                .map(|s| s.chars().take(options.snippet_chars()).collect::<String>())
                .unwrap_or_default()
        } else {
            snippet.to_html()
        };
        hits.push(SearchHit {
            key,
            title,
            snippet: snippet_text,
            score,
            source: None,
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
    fn limit_caps_the_number_of_hits() {
        let dir = tempfile::tempdir().unwrap();
        let notes: Vec<IndexedNote> = (0..30)
            .map(|i| note(&format!("Note {i}"), "shared keyword in every note"))
            .collect();
        rebuild(dir.path(), &notes).unwrap();

        // Default: the browsable list a human wants.
        assert_eq!(query(dir.path(), "keyword").unwrap().len(), DEFAULT_LIMIT);
        // An agent asking for the best few gets exactly that.
        let few = query_with(dir.path(), "keyword", &SearchOptions::with_limit(3)).unwrap();
        assert_eq!(few.len(), 3);
    }

    #[test]
    fn limit_is_clamped_into_range() {
        let dir = tempfile::tempdir().unwrap();
        let notes: Vec<IndexedNote> = (0..5)
            .map(|i| note(&format!("Note {i}"), "shared keyword"))
            .collect();
        rebuild(dir.path(), &notes).unwrap();

        // Zero would mean "no results at all", which no caller can want.
        assert_eq!(
            query_with(dir.path(), "keyword", &SearchOptions::with_limit(0))
                .unwrap()
                .len(),
            1
        );
        // Asking for the moon returns what exists, not an error.
        let huge = SearchOptions::with_limit(usize::MAX);
        assert_eq!(huge.limit, MAX_LIMIT);
        assert_eq!(query_with(dir.path(), "keyword", &huge).unwrap().len(), 5);
    }

    #[test]
    fn snippet_chars_bounds_the_context_per_hit() {
        let dir = tempfile::tempdir().unwrap();
        let long_body = format!(
            "{} keyword {}",
            "filler ".repeat(80),
            "trailing ".repeat(80)
        );
        rebuild(dir.path(), &[note("Long", &long_body)]).unwrap();

        let tight = SearchOptions {
            limit: 5,
            snippet_chars: 40,
        };
        let wide = SearchOptions {
            limit: 5,
            snippet_chars: 300,
        };
        let tight_len = query_with(dir.path(), "keyword", &tight).unwrap()[0]
            .snippet
            .chars()
            .count();
        let wide_len = query_with(dir.path(), "keyword", &wide).unwrap()[0]
            .snippet
            .chars()
            .count();
        assert!(
            tight_len < wide_len,
            "a smaller budget must produce a shorter snippet: {tight_len} vs {wide_len}"
        );
    }

    #[test]
    fn hits_carry_a_score_for_cross_vault_ranking() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[
                note("Strong", "keyword keyword keyword"),
                note(
                    "Weak",
                    "keyword buried among many other unrelated words here",
                ),
            ],
        )
        .unwrap();

        let hits = query(dir.path(), "keyword").unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits[0].score > 0.0, "score must be populated");
        assert!(
            hits[0].score >= hits[1].score,
            "results already arrive ranked"
        );
    }

    fn degrees(pairs: &[(&str, usize)]) -> std::collections::HashMap<String, usize> {
        pairs
            .iter()
            .map(|(key, degree)| ((*key).to_string(), *degree))
            .collect()
    }

    /// The boost has to be bounded and monotonic, or one hub note answers every
    /// query in the vault.
    #[test]
    fn degree_boost_is_bounded_and_monotonic() {
        assert_eq!(degree_boost(0), 1.0, "unconnected notes are not penalised");
        let ceiling = 1.0 + DEGREE_WEIGHT;
        for degree in [1usize, 3, 12, 40, 1000, usize::MAX] {
            let boost = degree_boost(degree);
            assert!(
                (1.0..=ceiling).contains(&boost),
                "degree {degree} produced {boost}, outside 1.0..={ceiling}"
            );
        }
        assert!(degree_boost(1) < degree_boost(3));
        assert!(degree_boost(3) < degree_boost(12));
        // Saturated: fifty links must not beat twelve by a meaningful margin.
        assert_eq!(degree_boost(12), degree_boost(50));
    }

    /// The point of the feature: among notes the query cannot tell apart, the one
    /// the rest of the vault points at wins — even from outside the cut. Ranked
    /// last by tie-break, it comes back first.
    #[test]
    fn connectedness_decides_between_equally_relevant_notes() {
        let dir = tempfile::tempdir().unwrap();
        let notes: Vec<IndexedNote> = ["A", "B", "C", "D", "E", "F"]
            .iter()
            .map(|n| note(n, "identical body mentioning keyword once"))
            .collect();
        rebuild(dir.path(), &notes).unwrap();

        let plain = query_with(dir.path(), "keyword", &SearchOptions::with_limit(2)).unwrap();
        assert_eq!(plain.len(), 2);
        assert!(
            !plain.iter().any(|h| h.key == "F.md"),
            "F must be outside the cut before ranking: {:?}",
            plain.iter().map(|h| &h.key).collect::<Vec<_>>()
        );

        let ranked = query_ranked(
            dir.path(),
            "keyword",
            &SearchOptions::with_limit(2),
            &degrees(&[("F.md", 30)]),
        )
        .unwrap();
        assert_eq!(ranked.len(), 2, "the limit is still the limit");
        assert_eq!(ranked[0].key, "F.md", "the connected note is promoted");
    }

    /// And the guard on the other side: connectedness is a hint, not a veto. A
    /// note that plainly matches the words beats a popular one that barely does.
    #[test]
    fn relevance_still_outranks_connectedness() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[
                note("Strong", &"keyword ".repeat(10)),
                note(
                    "Popular",
                    &format!("keyword {}", "unrelated filler words ".repeat(20)),
                ),
            ],
        )
        .unwrap();

        let ranked = query_ranked(
            dir.path(),
            "keyword",
            &SearchOptions::with_limit(2),
            &degrees(&[("Popular.md", 500)]),
        )
        .unwrap();
        assert_eq!(
            ranked[0].key,
            "Strong.md",
            "a weak match cannot win on popularity: {:?}",
            ranked.iter().map(|h| (&h.key, h.score)).collect::<Vec<_>>()
        );
    }

    /// An empty degree map is the "graph not built yet" case, and must behave
    /// exactly like plain relevance rather than erroring or reordering.
    #[test]
    fn ranking_without_degrees_matches_plain_relevance() {
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            dir.path(),
            &[
                note("One", "keyword keyword keyword"),
                note("Two", "keyword mentioned once here"),
                note("Three", "keyword keyword"),
            ],
        )
        .unwrap();

        let options = SearchOptions::with_limit(3);
        let plain: Vec<String> = query_with(dir.path(), "keyword", &options)
            .unwrap()
            .into_iter()
            .map(|h| h.key)
            .collect();
        let ranked: Vec<String> = query_ranked(dir.path(), "keyword", &options, &degrees(&[]))
            .unwrap()
            .into_iter()
            .map(|h| h.key)
            .collect();
        assert_eq!(plain, ranked);
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
