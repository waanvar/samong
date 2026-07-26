use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use redb::{
    Database, MultimapTableDefinition, ReadableMultimapTable, ReadableTable, TableDefinition,
};

use crate::vault::BRAIN_DIR;

/// note key -> raw wikilink target. Keys are vault-relative paths (see
/// [`crate::vault::relative_key`]); targets are the raw text inside `[[...]]`,
/// which may or may not name an existing note.
const FORWARD: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("forward");
/// raw wikilink target -> key of the note that links to it.
const BACKWARD: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("backward");
/// note key -> (file mtime in nanoseconds, blake3 content hash), used to detect
/// changed notes during incremental reindex.
const FILES: TableDefinition<&str, (u64, &str)> = TableDefinition::new("files");
/// title -> keys of every note carrying it. A title is a display name, not an
/// identity, so this is one-to-many by design.
const TITLES: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("titles");
/// Index metadata (key "index_version"): bumped when the schema/tokenizer
/// changes so stale indexes get rebuilt automatically.
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
const INDEX_VERSION_KEY: &str = "index_version";
/// Title-keyed mtime table from before notes were keyed by path. Only ever
/// deleted, never read.
const LEGACY_MTIMES: TableDefinition<&str, u64> = TableDefinition::new("mtimes");

/// The display title a note key implies: its filename without the `.md`.
/// Derived, never stored — the key is the only identity.
pub fn title_from_key(key: &str) -> Option<String> {
    let name = key.rsplit('/').next()?;
    Some(name.strip_suffix(".md").unwrap_or(name).to_string())
}

/// One note's contribution to the graph, identified by its vault-relative path.
pub struct NoteUpdate {
    /// Stable identity: vault-relative, slash-separated path.
    pub key: String,
    /// Display name (filename without `.md`). Not unique within a vault.
    pub title: String,
    pub targets: Vec<String>,
    pub mtime: u64,
    /// blake3 hash of the file contents. Survives anything that rewrites mtime
    /// without changing bytes — a `git checkout` being the case that matters.
    pub hash: String,
}

/// What was recorded about a file at index time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileState {
    pub mtime: u64,
    pub hash: String,
}

/// Link graph backed by redb: forward links (raw wikilink targets, which may or may not
/// correspond to a real note) and backlinks (their inverse).
pub struct Graph {
    db: Database,
}

impl Graph {
    pub fn open(vault: &Path) -> Result<Self> {
        let brain_dir = vault.join(BRAIN_DIR);
        fs::create_dir_all(&brain_dir)
            .with_context(|| format!("creating index dir {}", brain_dir.display()))?;
        let db_path = brain_dir.join("graph.redb");
        let db = Database::create(&db_path)
            .with_context(|| format!("opening graph db {}", db_path.display()))?;
        Ok(Self { db })
    }

    /// Apply an incremental batch in one transaction: replace the outgoing
    /// edges of each upserted note and drop removed notes entirely. `removals`
    /// are note keys.
    ///
    /// Backlinks pointing *at* a removed note are kept — the source files
    /// still contain those wikilinks (surfaced by `broken`).
    pub fn apply(&self, upserts: &[NoteUpdate], removals: &[String]) -> Result<()> {
        let txn = self
            .db
            .begin_write()
            .context("beginning write transaction")?;
        {
            let mut forward = txn.open_multimap_table(FORWARD)?;
            let mut backward = txn.open_multimap_table(BACKWARD)?;
            let mut titles = txn.open_multimap_table(TITLES)?;
            let mut files = txn.open_table(FILES)?;

            // Remove everything recorded for a note key, so re-adding it is a
            // clean insert rather than a merge with whatever was there before.
            let detach = |forward: &mut redb::MultimapTable<&str, &str>,
                          backward: &mut redb::MultimapTable<&str, &str>,
                          titles: &mut redb::MultimapTable<&str, &str>,
                          files: &mut redb::Table<&str, (u64, &str)>,
                          key: &str|
             -> Result<()> {
                let old_targets: Vec<String> = forward
                    .get(key)?
                    .map(|v| v.map(|g| g.value().to_string()))
                    .collect::<Result<_, _>>()?;
                for target in &old_targets {
                    backward.remove(target.as_str(), key)?;
                }
                forward.remove_all(key)?;
                // A rename changes the title a key maps to, so drop the old
                // title entry too. The title is derived from the key itself.
                if let Some(old_title) = title_from_key(key) {
                    titles.remove(old_title.as_str(), key)?;
                }
                files.remove(key)?;
                Ok(())
            };

            for key in removals {
                detach(&mut forward, &mut backward, &mut titles, &mut files, key)?;
            }
            for update in upserts {
                detach(
                    &mut forward,
                    &mut backward,
                    &mut titles,
                    &mut files,
                    &update.key,
                )?;
                for target in &update.targets {
                    forward.insert(update.key.as_str(), target.as_str())?;
                    backward.insert(target.as_str(), update.key.as_str())?;
                }
                titles.insert(update.title.as_str(), update.key.as_str())?;
                files.insert(update.key.as_str(), (update.mtime, update.hash.as_str()))?;
            }
        }
        txn.commit().context("committing graph update")?;
        Ok(())
    }

    /// Replace the entire graph with the given notes (full reindex).
    pub fn rebuild(&self, notes: &[NoteUpdate]) -> Result<()> {
        let txn = self
            .db
            .begin_write()
            .context("beginning write transaction")?;
        txn.delete_multimap_table(FORWARD)
            .context("clearing forward table")?;
        txn.delete_multimap_table(BACKWARD)
            .context("clearing backward table")?;
        txn.delete_multimap_table(TITLES)
            .context("clearing titles table")?;
        txn.delete_table(FILES).context("clearing files table")?;
        // Pre-0.2 vaults keyed notes by title in a "mtimes" table. Nothing
        // reads it any more; drop it so upgraded vaults carry no dead data.
        let _ = txn.delete_table(LEGACY_MTIMES);
        txn.commit().context("committing graph clear")?;
        self.apply(notes, &[])
    }

    /// The index-format version recorded when this vault was last indexed.
    pub fn index_version(&self) -> Result<Option<u64>> {
        let txn = self.db.begin_read().context("beginning read transaction")?;
        let table = match txn.open_table(META) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(table.get(INDEX_VERSION_KEY)?.map(|v| v.value()))
    }

    pub fn set_index_version(&self, version: u64) -> Result<()> {
        let txn = self
            .db
            .begin_write()
            .context("beginning write transaction")?;
        {
            let mut table = txn.open_table(META)?;
            table.insert(INDEX_VERSION_KEY, version)?;
        }
        txn.commit().context("committing index version")?;
        Ok(())
    }

    /// File state recorded at last index time, keyed by note key.
    pub fn stored_files(&self) -> Result<HashMap<String, FileState>> {
        let txn = self.db.begin_read().context("beginning read transaction")?;
        let table = match txn.open_table(FILES) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(HashMap::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = HashMap::new();
        for entry in table.iter()? {
            let (key, state) = entry?;
            let (mtime, hash) = state.value();
            out.insert(
                key.value().to_string(),
                FileState {
                    mtime,
                    hash: hash.to_string(),
                },
            );
        }
        Ok(out)
    }

    /// Outgoing link targets of one specific note, by key.
    pub fn forward_links(&self, key: &str) -> Result<Vec<String>> {
        self.lookup(FORWARD, key)
    }

    /// Keys of the notes that link to `target` (the raw text inside `[[...]]`,
    /// so a title or a `vault/title` cross-vault reference).
    pub fn backlinks(&self, target: &str) -> Result<Vec<String>> {
        self.lookup(BACKWARD, target)
    }

    /// Keys of every note carrying `title`, in sorted order. Empty if the title
    /// names nothing; more than one entry means the title is ambiguous.
    pub fn keys_for_title(&self, title: &str) -> Result<Vec<String>> {
        self.lookup(TITLES, title)
    }

    /// The union of the outgoing targets of every note carrying `title`.
    ///
    /// Front-ends address notes by title, which usually names exactly one file;
    /// when it names several, showing all their links beats silently picking one.
    pub fn forward_links_for_title(&self, title: &str) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for key in self.keys_for_title(title)? {
            out.extend(self.forward_links(&key)?);
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// Titles shared by more than one note, with their keys. Reported by
    /// `banyan doctor`: a title collision is legal but means `[[title]]` links
    /// and title-addressed API calls are ambiguous.
    pub fn duplicate_titles(&self) -> Result<Vec<(String, Vec<String>)>> {
        let txn = self.db.begin_read().context("beginning read transaction")?;
        let table = match txn.open_multimap_table(TITLES) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (title, keys) = entry?;
            let keys: Vec<String> = keys
                .map(|k| k.map(|g| g.value().to_string()))
                .collect::<Result<_, _>>()?;
            if keys.len() > 1 {
                out.push((title.value().to_string(), keys));
            }
        }
        out.sort();
        Ok(out)
    }

    fn lookup(
        &self,
        table_def: MultimapTableDefinition<&str, &str>,
        key: &str,
    ) -> Result<Vec<String>> {
        let txn = self.db.begin_read().context("beginning read transaction")?;
        let table = match txn.open_multimap_table(table_def) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for value in table.get(key)? {
            out.push(value?.value().to_string());
        }
        out.sort();
        Ok(out)
    }

    /// Every `(source key, raw target)` pair in the graph.
    pub fn all_edges(&self) -> Result<Vec<(String, String)>> {
        let txn = self.db.begin_read().context("beginning read transaction")?;
        let table = match txn.open_multimap_table(FORWARD) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (from, targets) = entry?;
            let from = from.value().to_string();
            for target in targets {
                out.push((from.clone(), target?.value().to_string()));
            }
        }
        out.sort();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A note at `<key>` linking to `targets`. The key carries the identity;
    /// the title is derived from it, exactly as the indexer does.
    fn update(key: &str, targets: &[&str]) -> NoteUpdate {
        NoteUpdate {
            key: key.to_string(),
            title: title_from_key(key).unwrap(),
            targets: targets.iter().map(|t| t.to_string()).collect(),
            mtime: 1,
            hash: format!("hash-of-{key}"),
        }
    }

    #[test]
    fn rebuild_and_query_links() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[update("B.md", &["A"]), update("C.md", &["A"])])
            .unwrap();

        assert_eq!(graph.forward_links("B.md").unwrap(), vec!["A".to_string()]);
        // Backlinks are keyed by the raw target text and return source keys.
        assert_eq!(
            graph.backlinks("A").unwrap(),
            vec!["B.md".to_string(), "C.md".to_string()]
        );
        assert!(graph.backlinks("B").unwrap().is_empty());
    }

    #[test]
    fn rebuild_clears_previous_state() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph.rebuild(&[update("X.md", &["Y"])]).unwrap();
        assert_eq!(graph.forward_links("X.md").unwrap(), vec!["Y".to_string()]);

        graph.rebuild(&[]).unwrap();
        assert!(graph.forward_links("X.md").unwrap().is_empty());
        assert!(graph.backlinks("Y").unwrap().is_empty());
        assert!(graph.stored_files().unwrap().is_empty());
        assert!(graph.keys_for_title("X").unwrap().is_empty());
    }

    #[test]
    fn all_edges_returns_every_pair() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[update("B.md", &["A"]), update("docs/C.md", &["A"])])
            .unwrap();
        assert_eq!(
            graph.all_edges().unwrap(),
            vec![
                ("B.md".to_string(), "A".to_string()),
                ("docs/C.md".to_string(), "A".to_string())
            ]
        );
    }

    #[test]
    fn empty_graph_has_no_edges() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        assert!(graph.all_edges().unwrap().is_empty());
        assert!(graph.forward_links("Anything.md").unwrap().is_empty());
        assert!(graph.duplicate_titles().unwrap().is_empty());
    }

    #[test]
    fn apply_replaces_only_touched_notes_edges() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[update("B.md", &["A", "C"]), update("D.md", &["A"])])
            .unwrap();

        // B now links only to C; D's edges must survive untouched.
        graph.apply(&[update("B.md", &["C"])], &[]).unwrap();

        assert_eq!(graph.forward_links("B.md").unwrap(), vec!["C".to_string()]);
        assert_eq!(graph.backlinks("A").unwrap(), vec!["D.md".to_string()]);
        assert_eq!(graph.backlinks("C").unwrap(), vec!["B.md".to_string()]);
    }

    #[test]
    fn apply_removal_drops_outgoing_but_keeps_incoming() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[update("A.md", &["X"]), update("B.md", &["A"])])
            .unwrap();

        graph.apply(&[], &["A.md".to_string()]).unwrap();

        // A's own outgoing edge is gone...
        assert!(graph.forward_links("A.md").unwrap().is_empty());
        assert!(graph.backlinks("X").unwrap().is_empty());
        // ...but B's file still says [[A]], so that edge stays (dangling).
        assert_eq!(graph.backlinks("A").unwrap(), vec!["B.md".to_string()]);
        assert!(!graph.stored_files().unwrap().contains_key("A.md"));
        assert!(graph.keys_for_title("A").unwrap().is_empty());
    }

    #[test]
    fn stored_files_round_trip_mtime_and_hash() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .apply(
                &[NoteUpdate {
                    key: "docs/A.md".to_string(),
                    title: "A".to_string(),
                    targets: vec![],
                    mtime: 42,
                    hash: "abc123".to_string(),
                }],
                &[],
            )
            .unwrap();
        let files = graph.stored_files().unwrap();
        assert_eq!(
            files.get("docs/A.md"),
            Some(&FileState {
                mtime: 42,
                hash: "abc123".to_string()
            })
        );
    }

    /// The regression that motivated keying by path: a repo full of `README.md`
    /// files used to collapse into a single graph node.
    #[test]
    fn same_title_in_different_directories_stays_separate() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[
                update("README.md", &["Root Target"]),
                update("docs/README.md", &["Docs Target"]),
                update("api/README.md", &["Api Target"]),
            ])
            .unwrap();

        // Each file keeps its own edges instead of overwriting its namesakes.
        assert_eq!(
            graph.forward_links("docs/README.md").unwrap(),
            vec!["Docs Target".to_string()]
        );
        assert_eq!(
            graph.forward_links("README.md").unwrap(),
            vec!["Root Target".to_string()]
        );
        assert_eq!(graph.stored_files().unwrap().len(), 3);

        // The shared title resolves to all three, and is reported as ambiguous.
        assert_eq!(
            graph.keys_for_title("README").unwrap(),
            vec![
                "README.md".to_string(),
                "api/README.md".to_string(),
                "docs/README.md".to_string()
            ]
        );
        assert_eq!(
            graph.forward_links_for_title("README").unwrap(),
            vec![
                "Api Target".to_string(),
                "Docs Target".to_string(),
                "Root Target".to_string()
            ]
        );
        let dupes = graph.duplicate_titles().unwrap();
        assert_eq!(dupes.len(), 1);
        assert_eq!(dupes[0].0, "README");
        assert_eq!(dupes[0].1.len(), 3);
    }

    #[test]
    fn unique_titles_are_not_reported_as_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[update("A.md", &[]), update("docs/B.md", &[])])
            .unwrap();
        assert!(graph.duplicate_titles().unwrap().is_empty());
    }

    #[test]
    fn title_from_key_takes_the_filename() {
        assert_eq!(title_from_key("docs/API.md").as_deref(), Some("API"));
        assert_eq!(title_from_key("A.md").as_deref(), Some("A"));
        assert_eq!(
            title_from_key("a/b/Deep Note.md").as_deref(),
            Some("Deep Note")
        );
    }
}
