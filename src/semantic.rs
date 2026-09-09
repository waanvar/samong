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

/// The one model this process loads, and the query it embedded last.
///
/// [`rank_by_similarity`] used to call [`Embedder::load`] itself, once per call —
/// and [`crate::ops::search_vault`] calls it once per vault. Searching five
/// vaults therefore loaded 465 MB of model five times to answer one question,
/// embedded the same query string five times, and threw all five away. The CLI
/// paid that per search; `samong-server` and `samong-mcp` are long-lived, so they
/// paid it on every request for as long as they ran.
///
/// The cost of holding it is real and worth stating: once a process has answered
/// one semantic search, the model stays resident in it. For the CLI that is the
/// length of one command. For the servers it is the rest of their life — which is
/// the trade being made deliberately, because the alternative was paying the load
/// again on every request.
///
/// A `Mutex` rather than a per-thread copy: the HTTP server answers on a pool of
/// blocking threads, and one model per thread is the same waste again, spread out
/// where it is harder to see.
static SHARED: std::sync::Mutex<Option<Shared>> = std::sync::Mutex::new(None);

struct Shared {
    embedder: Embedder,
    queries: QueryCache,
}

/// The last query vector, kept because a multi-vault search asks for the same one
/// once per vault.
///
/// One entry rather than a map, on purpose. The repetition being removed is
/// immediate — the same string, once per vault, inside one search — and a map
/// would instead hold a vector for every query a server had ever been asked,
/// growing without a bound anybody chose.
#[derive(Default)]
struct QueryCache {
    last: Option<(String, Vec<f32>)>,
}

impl QueryCache {
    /// The vector for `text`, computing it only when it is not the one just done.
    ///
    /// A failed embed leaves the previous entry alone rather than clearing it: the
    /// old entry is still correct for the old query, and dropping it would make an
    /// error cost the *next* search a recomputation too.
    fn get_or_insert_with(
        &mut self,
        text: &str,
        embed: impl FnOnce(&str) -> Result<Vec<f32>>,
    ) -> Result<Vec<f32>> {
        if let Some((cached, vector)) = &self.last {
            if cached == text {
                return Ok(vector.clone());
            }
        }
        let vector = embed(text)?;
        self.last = Some((text.to_string(), vector.clone()));
        Ok(vector)
    }
}

/// Embed a search query, loading the model at most once per process.
fn query_vector(text: &str) -> Result<Vec<f32>> {
    // A poisoned lock means an earlier call panicked while holding it. What is
    // behind the lock is a cache, not an invariant, so taking it back is strictly
    // better than failing every later search on account of one old panic.
    let mut guard = SHARED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.is_none() {
        *guard = Some(Shared {
            // No progress bar: this is a search, and the caller is waiting on a
            // result rather than watching a download. `samong embed` is where a
            // first download is expected and shown.
            embedder: Embedder::load(false)?,
            queries: QueryCache::default(),
        });
    }
    let shared = guard.as_mut().expect("stored just above");
    // Split the borrow: the closure needs the embedder while the cache is held.
    let Shared { embedder, queries } = shared;
    queries.get_or_insert_with(text, |text| embedder.embed_query(text))
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
/// May load the model — once per process, see [`SHARED`] — so this is still only
/// worth calling when [`crate::vectors::exists`] says the vault has something to
/// compare against: a vault with no vectors should not pay for a model at all.
pub fn rank_by_similarity(vault: &Path, text: &str, limit: usize) -> Result<Vec<String>> {
    let store = Store::open(vault)?;
    let entries = store.all()?;
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let query = query_vector(text)?;

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

    /// The defect: `ops::search_vault` runs once per vault, and each run embedded
    /// the query again. Five vaults, five identical embeds of the same string.
    ///
    /// No model here on purpose — the counting closure stands in for it, so this
    /// runs in CI, which deliberately stops short of downloading 465 MB.
    #[test]
    fn the_same_query_is_embedded_once_however_many_vaults_ask() {
        let mut cache = QueryCache::default();
        let mut embeds = 0;
        let mut ask = |cache: &mut QueryCache, text: &str, embeds: &mut usize| {
            cache
                .get_or_insert_with(text, |_| {
                    *embeds += 1;
                    Ok(vec![0.5, 0.25])
                })
                .unwrap()
        };

        // One query, five vaults.
        for _ in 0..5 {
            assert_eq!(ask(&mut cache, "ตลาดหลักทรัพย์", &mut embeds), vec![0.5, 0.25]);
        }
        assert_eq!(embeds, 1, "the query was embedded once per vault");

        // A different query is not answered from the old one.
        ask(&mut cache, "something else", &mut embeds);
        assert_eq!(embeds, 2);

        // And the cache moved on: the first query costs again, which is the price
        // of holding one entry rather than an unbounded map.
        ask(&mut cache, "ตลาดหลักทรัพย์", &mut embeds);
        assert_eq!(embeds, 3);
    }

    /// A failed embed must not throw away an entry that is still correct, and must
    /// not leave the failed query behind as if it had succeeded.
    #[test]
    fn a_failed_embed_leaves_the_previous_entry_intact() {
        let mut cache = QueryCache::default();
        cache
            .get_or_insert_with("first", |_| Ok(vec![1.0]))
            .unwrap();

        assert!(cache
            .get_or_insert_with("second", |_| Err(anyhow::anyhow!("model went away")))
            .is_err());

        let mut embeds = 0;
        let again = cache
            .get_or_insert_with("first", |_| {
                embeds += 1;
                Ok(vec![9.9])
            })
            .unwrap();
        assert_eq!(again, vec![1.0], "the surviving entry was replaced");
        assert_eq!(embeds, 0, "the surviving entry was recomputed");

        // The failed query was not recorded as if it had worked.
        let mut embeds = 0;
        cache
            .get_or_insert_with("second", |_| {
                embeds += 1;
                Ok(vec![2.0])
            })
            .unwrap();
        assert_eq!(embeds, 1);
    }

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
