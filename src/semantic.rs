//! Local embeddings, for ranking notes by meaning as well as by words.
//!
//! Compiled only with the `semantic` feature. The reason it is optional is not
//! caution about the code but honesty about the trade: this pulls in ONNX Runtime
//! and the first run downloads a model from Hugging Face. Notes never leave the
//! machine and no query is ever sent anywhere — but "one binary, nothing to
//! fetch" stops being true, and that is a promise worth protecting for everyone
//! who does not need this.
//!
//! **The model is multilingual on purpose.** Lexical Thai search is the thing
//! Samong does that others do not, and the nearest comparable project embeds with
//! an English-only model — semantic search that cannot read Thai would hand that
//! advantage away in the one place it matters most. `multilingual-e5-small`
//! covers 100+ languages at 384 dimensions, which is small enough to embed a
//! few thousand notes on a laptop CPU.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use crate::vectors::{self, Store};

/// The model this build embeds with. Written into every vector store, which
/// refuses to mix vectors from two models.
pub const MODEL_NAME: &str = "intfloat/multilingual-e5-small";
/// Vector width of that model.
pub const MODEL_DIM: usize = 384;

/// Characters per chunk.
///
/// The model reads at most 512 tokens. Thai runs about two to three characters
/// per token in a multilingual tokenizer where English runs four to five, so a
/// budget that is safe for Thai wastes some capacity on English — the right way
/// round, because silently dropping the tail of a Thai note is the failure that
/// would be hardest to notice.
const CHUNK_CHARS: usize = 900;
/// Chunks per note. Generous rather than tight: a 48 KB design document is a real
/// note, not an abuse. This exists so one pathological file cannot turn an embed
/// run into an afternoon.
const MAX_CHUNKS: usize = 200;
/// Documents per forward pass.
const BATCH: usize = 32;

/// E5 models are trained with these prefixes and lose accuracy without them: the
/// same sentence embedded as a query and as a passage should not land in the same
/// place, and the prefix is how the model is told which it is looking at.
fn as_passage(text: &str) -> String {
    format!("passage: {text}")
}

fn as_query(text: &str) -> String {
    format!("query: {text}")
}

/// Where downloaded model files live: with the rest of Samong's machine-local
/// state, not in the current directory (fastembed's own default would drop a
/// cache folder into whatever vault you happened to be standing in).
fn model_cache_dir() -> Result<PathBuf> {
    Ok(crate::registry::config_dir()?.join("models"))
}

/// Split a note into overlapping-free chunks on paragraph boundaries where it can.
///
/// Splitting mid-sentence produces an embedding of half a thought, so the split
/// walks back to the last blank line or newline inside the budget before giving
/// up and cutting at the character limit.
pub fn chunk(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < chars.len() && out.len() < MAX_CHUNKS {
        let hard_end = (start + CHUNK_CHARS).min(chars.len());
        let mut end = hard_end;
        if hard_end < chars.len() {
            // Prefer a paragraph break, then any newline, but never cut so early
            // that the chunk is mostly empty.
            let floor = start + CHUNK_CHARS / 2;
            if let Some(pos) = (floor..hard_end)
                .rev()
                .find(|&i| chars[i] == '\n' && i > 0 && chars[i - 1] == '\n')
            {
                end = pos;
            } else if let Some(pos) = (floor..hard_end).rev().find(|&i| chars[i] == '\n') {
                end = pos;
            }
        }
        let piece: String = chars[start..end].iter().collect();
        if !piece.trim().is_empty() {
            out.push(piece.trim().to_string());
        }
        start = end.max(start + 1);
    }
    out
}

/// A loaded model. Holding one is expensive, so callers keep it for a whole run.
pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    /// Load the model, downloading it on first use.
    pub fn load(show_progress: bool) -> Result<Self> {
        let options = InitOptions::new(EmbeddingModel::MultilingualE5Small)
            .with_cache_dir(model_cache_dir()?)
            .with_show_download_progress(show_progress);
        let model = TextEmbedding::try_new(options)
            .context("loading the embedding model (first run downloads it)")?;
        Ok(Self { model })
    }

    /// Embed one note's text as passages, one vector per chunk.
    pub fn embed_note(&mut self, title: &str, body: &str) -> Result<Vec<Vec<f32>>> {
        // The title rides along on the first chunk: it is often the most
        // informative sentence in the file and would otherwise only be searchable
        // lexically.
        let mut pieces = chunk(body);
        if pieces.is_empty() {
            pieces.push(title.to_string());
        } else {
            pieces[0] = format!("{title}\n\n{}", pieces[0]);
        }
        let prefixed: Vec<String> = pieces.iter().map(|p| as_passage(p)).collect();
        self.model
            .embed(&prefixed, Some(BATCH))
            .context("embedding note text")
    }

    /// Embed a search query.
    pub fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        let mut vectors = self
            .model
            .embed(&[as_query(text)], Some(1))
            .context("embedding the query")?;
        vectors
            .pop()
            .context("the embedding model returned nothing for the query")
    }
}

/// What one embed run did.
pub struct EmbedReport {
    pub embedded: usize,
    /// Already had an up-to-date vector, so nothing was recomputed.
    pub unchanged: usize,
    /// Notes whose vectors were dropped because the file is gone or left scope.
    pub removed: usize,
    /// Reference notes passed over because the run did not ask for them.
    pub skipped_reference: usize,
    pub total: usize,
}

/// Embed every note in a vault that does not already have a current vector.
///
/// Kept out of `reindex` on purpose. Reindexing is expected to be instant and to
/// work offline; embedding is neither — it needs a model on disk and takes real
/// time per note. Folding it in would make every save unpredictable. So this is
/// an explicit step, and search uses whatever it finds.
///
/// Notes are compared by the blake3 hash the indexer already computes, so a second
/// run over an unchanged vault embeds nothing.
///
/// `include_reference` decides whether vendored documentation is embedded too. It
/// defaults to off in the CLI, from a measurement: embedding this project's own
/// test vault took 11m25s, and 425 of its 430 notes were Next.js documentation —
/// 95% of the wait for material that is somebody else's reference manual, still
/// fully searchable by words. Same judgement the graph makes when it hides
/// reference notes by default, for the same reason.
///
/// Turning it off never *deletes* vectors it previously wrote: reference notes are
/// still in scope, and silently throwing away eleven minutes of work because a
/// flag moved would be its own kind of rude.
pub fn embed_vault(
    scope: &crate::scope::Scope,
    include_reference: bool,
    show_progress: bool,
) -> Result<EmbedReport> {
    let vault = scope.root();
    let notes = crate::vault::list_notes_in(scope)?;
    let graph = crate::graph::Graph::open(vault)?;
    // The indexer already hashed every file; reusing those hashes keeps one
    // definition of "changed" across the whole program.
    let indexed = graph.stored_files()?;

    let store = Store::open(vault)?;
    store.claim(MODEL_NAME, MODEL_DIM)?;
    let stored = store.stored_hashes()?;

    let mut pending = Vec::new();
    let mut unchanged = 0usize;
    let mut skipped_reference = 0usize;
    for note in &notes {
        let Some(state) = indexed.get(&note.key) else {
            // Not in the text index yet: `samong reindex` owns that, and embedding
            // a note the rest of the program does not know about would produce a
            // hit that cannot be opened.
            continue;
        };
        let current = stored
            .get(&note.key)
            .is_some_and(|hash| hash == &state.hash);
        if current {
            unchanged += 1;
            continue;
        }
        // Counted only when there is work being declined, so the number means
        // "this is what asking for reference notes would add".
        if note.reference && !include_reference {
            skipped_reference += 1;
            continue;
        }
        pending.push((note.key.clone(), note.title.clone(), state.hash.clone()));
    }

    let live: std::collections::HashSet<&String> = notes.iter().map(|n| &n.key).collect();
    let removals: Vec<String> = stored
        .keys()
        .filter(|key| !live.contains(key))
        .cloned()
        .collect();

    if pending.is_empty() {
        store.apply(&[], &removals)?;
        return Ok(EmbedReport {
            embedded: 0,
            unchanged,
            removed: removals.len(),
            skipped_reference,
            total: notes.len(),
        });
    }

    let mut embedder = Embedder::load(show_progress)?;
    let mut upserts = Vec::with_capacity(pending.len());
    for (key, title, hash) in pending {
        let path = vault.join(&key);
        let body = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let chunks = embedder.embed_note(&title, &body)?;
        upserts.push((key, hash, chunks));
    }
    let embedded = upserts.len();
    store.apply(&upserts, &removals)?;
    Ok(EmbedReport {
        embedded,
        unchanged,
        removed: removals.len(),
        skipped_reference,
        total: notes.len(),
    })
}

/// Note keys ordered by semantic similarity to `text`, best first.
///
/// A note scores as its best-matching chunk: a long document that answers the
/// question in one section is a good answer, and averaging over its other
/// sections would bury that.
///
/// Loads the model, so this is only worth calling when [`crate::vectors::exists`]
/// says the vault has something to compare against.
pub fn rank_by_similarity(vault: &Path, text: &str, limit: usize) -> Result<Vec<String>> {
    let store = Store::open(vault)?;
    let entries = store.all()?;
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let mut embedder = Embedder::load(false)?;
    let query = embedder.embed_query(text)?;

    let mut scored: Vec<(String, f32)> = entries
        .into_iter()
        .map(|entry| {
            let best = entry
                .chunks
                .iter()
                .map(|chunk| vectors::cosine(&query, chunk))
                .fold(f32::MIN, f32::max);
            (entry.key, best)
        })
        .filter(|(_, score)| *score > f32::MIN)
        .collect();
    // Ties broken by key, so two identical notes always come back in one order.
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    scored.truncate(limit);
    Ok(scored.into_iter().map(|(key, _)| key).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_produces_no_chunks() {
        assert!(chunk("").is_empty());
        assert!(chunk("   \n\n  ").is_empty());
    }

    #[test]
    fn short_text_is_one_chunk() {
        let pieces = chunk("# Title\n\nA short note about deployment.");
        assert_eq!(pieces.len(), 1);
        assert!(pieces[0].contains("deployment"));
    }

    /// The whole note has to be covered — a chunker that drops the tail loses
    /// knowledge silently, which is the worst way to lose it.
    #[test]
    fn long_text_is_split_and_nothing_is_lost() {
        let paragraph = "Sentence about a specific topic. ".repeat(20);
        let body = (0..10)
            .map(|i| format!("Paragraph {i}. {paragraph}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let pieces = chunk(&body);
        assert!(pieces.len() > 1, "expected several chunks");
        for i in 0..10 {
            let marker = format!("Paragraph {i}.");
            assert!(
                pieces.iter().any(|p| p.contains(&marker)),
                "{marker} went missing from the chunks"
            );
        }
        for piece in &pieces {
            assert!(
                piece.chars().count() <= CHUNK_CHARS,
                "a chunk exceeded the model's budget: {} chars",
                piece.chars().count()
            );
        }
    }

    /// Thai has no spaces, so a chunker that only breaks on whitespace would
    /// either produce one giant chunk or cut blindly.
    #[test]
    fn thai_text_without_spaces_still_chunks_within_budget() {
        let line = "ตลาดหลักทรัพย์แห่งประเทศไทยประกาศดัชนีใหม่เมื่อวานนี้".repeat(30);
        let body = format!("{line}\n\n{line}");
        let pieces = chunk(&body);
        assert!(pieces.len() > 1);
        for piece in &pieces {
            assert!(piece.chars().count() <= CHUNK_CHARS);
        }
    }

    #[test]
    fn chunk_count_is_capped() {
        let body = "x\n\n".repeat(CHUNK_CHARS * (MAX_CHUNKS + 50));
        assert!(chunk(&body).len() <= MAX_CHUNKS);
    }

    /// E5 needs the asymmetric prefixes; without them the same words as a query
    /// and as a passage embed to the same point and ranking degrades.
    #[test]
    fn queries_and_passages_are_prefixed_differently() {
        assert_eq!(as_query("rate limiting"), "query: rate limiting");
        assert_eq!(as_passage("rate limiting"), "passage: rate limiting");
    }
}
