//! Storage for note embeddings, used by semantic search.
//!
//! Deliberately its own redb file at `<vault>/.brain/vectors.redb` rather than a
//! table inside the link graph. Semantic search is an opt-in feature that not
//! everyone compiles in, and a separate file means turning it on or off never
//! migrates, locks or risks the index everything else depends on: delete the file
//! and the vault is exactly what it was.
//!
//! Vectors are keyed by note path and stamped with the note's blake3 content
//! hash — the same hash the incremental reindexer already computes — so
//! re-embedding skips notes that have not changed. Embedding is the slowest thing
//! this program does; not repeating it is the difference between usable and not.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use redb::{Database, ReadableTable, TableDefinition};

use crate::vault::BRAIN_DIR;

/// note key -> (content hash, chunk vectors packed as little-endian f32).
///
/// One row per note even when a note is long enough to need several chunks: the
/// unit of invalidation is the file, so the unit of storage should be too.
const VECTORS: TableDefinition<&str, (&str, &[u8])> = TableDefinition::new("vectors");
/// Which model wrote this store, and how wide its vectors are. Embeddings from
/// two different models are not comparable, so a model change has to invalidate
/// everything rather than silently mix.
const META: TableDefinition<&str, &str> = TableDefinition::new("meta");
const MODEL_KEY: &str = "model";
const DIM_KEY: &str = "dim";

pub struct Store {
    db: Database,
}

/// What the store holds for one note.
pub struct Entry {
    pub key: String,
    /// One vector per chunk. A note scores as its best-matching chunk.
    pub chunks: Vec<Vec<f32>>,
}

fn store_path(vault: &Path) -> std::path::PathBuf {
    vault.join(BRAIN_DIR).join("vectors.redb")
}

/// True when this vault has an embedding store at all. Search checks this before
/// paying for anything: a vault nobody ran `samong embed` on stays purely lexical.
pub fn exists(vault: &Path) -> bool {
    store_path(vault).exists()
}

fn pack(chunks: &[Vec<f32>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(chunks.iter().map(|c| c.len() * 4).sum());
    for chunk in chunks {
        for value in chunk {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out
}

fn unpack(bytes: &[u8], dim: usize) -> Vec<Vec<f32>> {
    if dim == 0 {
        return Vec::new();
    }
    // The outer split stays `chunks_exact`: `dim` is a runtime value. The inner
    // one is four bytes of an f32, a constant, so `as_chunks::<4>()` gives back
    // `&[[u8; 4]]` — which `from_le_bytes` takes directly, with no indexing and
    // no chance of the four subscripts drifting out of order.
    //
    // `.1` is the remainder, deliberately ignored: `chunks_exact` dropped a short
    // tail too, and a trailing partial float is a corrupt record either way.
    bytes
        .chunks_exact(dim * 4)
        .map(|chunk| {
            chunk
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect()
        })
        .collect()
}

impl Store {
    /// Open (or create) the store for a vault.
    pub fn open(vault: &Path) -> Result<Self> {
        let path = store_path(vault);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let db = Database::create(&path).with_context(|| format!("opening {}", path.display()))?;
        Ok(Self { db })
    }

    /// Record which model produced the vectors in here.
    ///
    /// Rejects a second model rather than mixing: cosine similarity between
    /// vectors from different models is a meaningless number that would quietly
    /// rank nonsense. The caller is told to rebuild, which is a real answer.
    pub fn claim(&self, model: &str, dim: usize) -> Result<()> {
        if let Some((existing_model, existing_dim)) = self.meta()? {
            if existing_model != model || existing_dim != dim {
                bail!(
                    "this vault's embeddings were built with {existing_model} ({existing_dim}d) \
                     and cannot be mixed with {model} ({dim}d) — delete \
                     <vault>/.brain/vectors.redb and embed again"
                );
            }
            return Ok(());
        }
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(META)?;
            table.insert(MODEL_KEY, model)?;
            table.insert(DIM_KEY, dim.to_string().as_str())?;
        }
        txn.commit()?;
        Ok(())
    }

    /// The model name and vector width this store was built with.
    pub fn meta(&self) -> Result<Option<(String, usize)>> {
        let txn = self.db.begin_read()?;
        let table = match txn.open_table(META) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let model = table.get(MODEL_KEY)?.map(|v| v.value().to_string());
        let dim = table
            .get(DIM_KEY)?
            .and_then(|v| v.value().parse::<usize>().ok());
        Ok(match (model, dim) {
            (Some(model), Some(dim)) => Some((model, dim)),
            _ => None,
        })
    }

    /// Content hash per note key, for deciding what still needs embedding.
    pub fn stored_hashes(&self) -> Result<HashMap<String, String>> {
        let txn = self.db.begin_read()?;
        let table = match txn.open_table(VECTORS) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(HashMap::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = HashMap::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            out.insert(key.value().to_string(), value.value().0.to_string());
        }
        Ok(out)
    }

    /// Insert or replace one note's chunk vectors, in a single transaction with
    /// any removals so the store never reflects half an update.
    pub fn apply(
        &self,
        upserts: &[(String, String, Vec<Vec<f32>>)],
        removals: &[String],
    ) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(VECTORS)?;
            for key in removals {
                table.remove(key.as_str())?;
            }
            for (key, hash, chunks) in upserts {
                let packed = pack(chunks);
                table.insert(key.as_str(), (hash.as_str(), packed.as_slice()))?;
            }
        }
        txn.commit()?;
        Ok(())
    }

    /// Every stored note with its vectors.
    ///
    /// Semantic search scans all of them and takes the best cosine per note. At a
    /// few thousand notes and 384 dimensions that is a couple of million
    /// multiply-adds — faster than the disk read that fetched it, and it avoids an
    /// approximate-nearest-neighbour index that would be another dependency and
    /// another thing to keep in sync. A vault big enough to need one has other
    /// problems first.
    pub fn all(&self) -> Result<Vec<Entry>> {
        let Some((_, dim)) = self.meta()? else {
            return Ok(Vec::new());
        };
        let txn = self.db.begin_read()?;
        let table = match txn.open_table(VECTORS) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            out.push(Entry {
                key: key.value().to_string(),
                chunks: unpack(value.value().1, dim),
            });
        }
        Ok(out)
    }

    /// How many notes have vectors. Reported by `samong doctor` so "semantic
    /// search found nothing" can be told apart from "nothing was embedded".
    pub fn count(&self) -> Result<usize> {
        let txn = self.db.begin_read()?;
        let table = match txn.open_table(VECTORS) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(0),
            Err(e) => return Err(e.into()),
        };
        Ok(table.iter()?.count())
    }
}

/// Cosine similarity of two vectors of equal length.
///
/// Not assuming pre-normalised input: the model normalises today, but a stored
/// vector that turns out not to be unit length would silently produce scores
/// above 1 and reorder everything, and dividing by the norms costs nothing here.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_of(values: &[f32]) -> Vec<f32> {
        values.to_vec()
    }

    /// `pack`/`unpack` are the on-disk format for every embedding, and they had no
    /// direct test — the store tests exercised them only in passing, through a
    /// path where a byte-order or stride mistake could still round-trip. That gap
    /// mattered the moment `unpack` was rewritten from `chunks_exact(4)` to
    /// `as_chunks::<4>()` to satisfy a clippy lint that only exists in newer
    /// toolchains: the change is meant to be exactly behaviour-preserving, and
    /// nothing here proved it.
    #[test]
    fn packing_survives_a_round_trip() {
        for dim in [1_usize, 3, 4, 384] {
            let chunks: Vec<Vec<f32>> = (0..5)
                .map(|c| {
                    (0..dim)
                        .map(|i| (c as f32) * 100.0 + (i as f32) * 0.25 - 7.5)
                        .collect()
                })
                .collect();
            let packed = pack(&chunks);
            assert_eq!(
                packed.len(),
                chunks.len() * dim * 4,
                "dim {dim}: byte count"
            );
            assert_eq!(unpack(&packed, dim), chunks, "dim {dim}: round trip");
        }
    }

    /// Values that a naive implementation can mangle: the sign bit, subnormals,
    /// and the ones where a wrong byte order still produces a valid float.
    #[test]
    fn packing_preserves_awkward_floats() {
        let awkward = vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            f32::MIN,
            f32::MAX,
            f32::MIN_POSITIVE,
            f32::EPSILON,
            1.0e-30,
            -3.402_823_5e38,
        ];
        let dim = awkward.len();
        // `from_ref` rather than `[awkward.clone()]`: the clone existed only to
        // build a one-element slice, which is what `from_ref` is for.
        let back = unpack(&pack(std::slice::from_ref(&awkward)), dim);
        assert_eq!(back.len(), 1);
        for (got, want) in back[0].iter().zip(&awkward) {
            // Bit equality, not `==`: it is the only comparison that separates
            // +0.0 from -0.0, which a byte-order bug can silently swap.
            assert_eq!(got.to_bits(), want.to_bits(), "{want} came back as {got}");
        }
    }

    /// A zero dimension has to return nothing rather than divide by it, and a
    /// trailing partial float is a corrupt record that must not become a value.
    #[test]
    fn packing_rejects_nonsense_lengths() {
        assert!(unpack(&[1, 2, 3, 4], 0).is_empty(), "dim 0");
        assert!(unpack(&[], 4).is_empty(), "no bytes");
        // Nine bytes cannot hold two 4-byte floats plus a whole one.
        assert_eq!(unpack(&[0; 9], 2).len(), 1, "the short tail is dropped");
    }

    #[test]
    fn round_trips_chunk_vectors() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.claim("test-model", 3).unwrap();
        store
            .apply(
                &[(
                    "A.md".to_string(),
                    "hash-a".to_string(),
                    vec![vec_of(&[1.0, 0.0, 0.0]), vec_of(&[0.0, 1.0, 0.0])],
                )],
                &[],
            )
            .unwrap();

        let all = store.all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].key, "A.md");
        assert_eq!(all[0].chunks.len(), 2, "both chunks survive");
        assert_eq!(all[0].chunks[0], vec_of(&[1.0, 0.0, 0.0]));
        assert_eq!(all[0].chunks[1], vec_of(&[0.0, 1.0, 0.0]));
        assert_eq!(
            store.stored_hashes().unwrap().get("A.md").unwrap(),
            "hash-a"
        );
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn removals_and_upserts_apply_together() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.claim("test-model", 2).unwrap();
        store
            .apply(
                &[
                    (
                        "A.md".to_string(),
                        "h1".to_string(),
                        vec![vec_of(&[1.0, 0.0])],
                    ),
                    (
                        "B.md".to_string(),
                        "h2".to_string(),
                        vec![vec_of(&[0.0, 1.0])],
                    ),
                ],
                &[],
            )
            .unwrap();

        store
            .apply(
                &[(
                    "A.md".to_string(),
                    "h3".to_string(),
                    vec![vec_of(&[0.5, 0.5])],
                )],
                &["B.md".to_string()],
            )
            .unwrap();

        let hashes = store.stored_hashes().unwrap();
        assert_eq!(
            hashes.get("A.md").unwrap(),
            "h3",
            "hash follows the content"
        );
        assert!(!hashes.contains_key("B.md"), "removal took effect");
    }

    /// Mixing two models' vectors would produce cosine numbers that mean nothing,
    /// so the store refuses rather than quietly ranking noise.
    #[test]
    fn a_different_model_is_refused_not_mixed() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.claim("model-a", 384).unwrap();
        assert!(store.claim("model-a", 384).is_ok(), "same model is fine");

        let err = store.claim("model-b", 384).unwrap_err().to_string();
        assert!(err.contains("model-b"), "{err}");
        assert!(
            err.contains("vectors.redb"),
            "must say how to fix it: {err}"
        );

        let err = store.claim("model-a", 768).unwrap_err().to_string();
        assert!(
            err.contains("768"),
            "a width change also invalidates: {err}"
        );
    }

    #[test]
    fn cosine_is_one_for_identical_and_zero_for_orthogonal() {
        let a = vec_of(&[1.0, 2.0, 3.0]);
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-6);
        assert!(cosine(&vec_of(&[1.0, 0.0]), &vec_of(&[0.0, 1.0])).abs() < 1e-6);
        // Length mismatch and empties are answered, not panicked on.
        assert_eq!(cosine(&vec_of(&[1.0]), &vec_of(&[1.0, 2.0])), 0.0);
        assert_eq!(cosine(&[], &[]), 0.0);
        // A zero vector has no direction, so it is similar to nothing.
        assert_eq!(cosine(&vec_of(&[0.0, 0.0]), &vec_of(&[1.0, 1.0])), 0.0);
    }

    #[test]
    fn an_unclaimed_store_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        assert!(store.meta().unwrap().is_none());
        assert!(store.all().unwrap().is_empty());
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn exists_reports_whether_a_vault_was_ever_embedded() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(BRAIN_DIR)).unwrap();
        assert!(!exists(dir.path()));
        let _store = Store::open(dir.path()).unwrap();
        assert!(exists(dir.path()));
    }
}
