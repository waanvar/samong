use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use redb::{
    Database, MultimapTableDefinition, ReadableMultimapTable, ReadableTable, TableDefinition,
};

use crate::vault::BRAIN_DIR;

const FORWARD: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("forward");
const BACKWARD: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("backward");
/// title -> file mtime (nanoseconds since epoch), used to detect changed notes
/// during incremental reindex.
const MTIMES: TableDefinition<&str, u64> = TableDefinition::new("mtimes");

/// One note's contribution to the graph: its outgoing link targets plus the
/// file mtime recorded at index time.
pub struct NoteUpdate {
    pub title: String,
    pub targets: Vec<String>,
    pub mtime: u64,
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
    /// edges of each upserted note and drop removed notes entirely.
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
            let mut mtimes = txn.open_table(MTIMES)?;

            let detach = |forward: &mut redb::MultimapTable<&str, &str>,
                          backward: &mut redb::MultimapTable<&str, &str>,
                          title: &str|
             -> Result<()> {
                let old_targets: Vec<String> = forward
                    .get(title)?
                    .map(|v| v.map(|g| g.value().to_string()))
                    .collect::<Result<_, _>>()?;
                for target in &old_targets {
                    backward.remove(target.as_str(), title)?;
                }
                forward.remove_all(title)?;
                Ok(())
            };

            for title in removals {
                detach(&mut forward, &mut backward, title)?;
                mtimes.remove(title.as_str())?;
            }
            for update in upserts {
                detach(&mut forward, &mut backward, &update.title)?;
                for target in &update.targets {
                    forward.insert(update.title.as_str(), target.as_str())?;
                    backward.insert(target.as_str(), update.title.as_str())?;
                }
                mtimes.insert(update.title.as_str(), update.mtime)?;
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
        txn.delete_table(MTIMES).context("clearing mtimes table")?;
        txn.commit().context("committing graph clear")?;
        self.apply(notes, &[])
    }

    /// mtimes recorded at last index time, keyed by title.
    pub fn stored_mtimes(&self) -> Result<HashMap<String, u64>> {
        let txn = self.db.begin_read().context("beginning read transaction")?;
        let table = match txn.open_table(MTIMES) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(HashMap::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = HashMap::new();
        for entry in table.iter()? {
            let (title, mtime) = entry?;
            out.insert(title.value().to_string(), mtime.value());
        }
        Ok(out)
    }

    pub fn forward_links(&self, title: &str) -> Result<Vec<String>> {
        self.lookup(FORWARD, title)
    }

    pub fn backlinks(&self, title: &str) -> Result<Vec<String>> {
        self.lookup(BACKWARD, title)
    }

    fn lookup(
        &self,
        table_def: MultimapTableDefinition<&str, &str>,
        title: &str,
    ) -> Result<Vec<String>> {
        let txn = self.db.begin_read().context("beginning read transaction")?;
        let table = match txn.open_multimap_table(table_def) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for value in table.get(title)? {
            out.push(value?.value().to_string());
        }
        out.sort();
        Ok(out)
    }

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

    fn update(title: &str, targets: &[&str]) -> NoteUpdate {
        NoteUpdate {
            title: title.to_string(),
            targets: targets.iter().map(|t| t.to_string()).collect(),
            mtime: 1,
        }
    }

    #[test]
    fn rebuild_and_query_links() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[update("B", &["A"]), update("C", &["A"])])
            .unwrap();

        assert_eq!(graph.forward_links("B").unwrap(), vec!["A".to_string()]);
        assert_eq!(
            graph.backlinks("A").unwrap(),
            vec!["B".to_string(), "C".to_string()]
        );
        assert!(graph.backlinks("B").unwrap().is_empty());
    }

    #[test]
    fn rebuild_clears_previous_state() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph.rebuild(&[update("X", &["Y"])]).unwrap();
        assert_eq!(graph.forward_links("X").unwrap(), vec!["Y".to_string()]);

        graph.rebuild(&[]).unwrap();
        assert!(graph.forward_links("X").unwrap().is_empty());
        assert!(graph.backlinks("Y").unwrap().is_empty());
        assert!(graph.stored_mtimes().unwrap().is_empty());
    }

    #[test]
    fn all_edges_returns_every_pair() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[update("B", &["A"]), update("C", &["A"])])
            .unwrap();
        assert_eq!(
            graph.all_edges().unwrap(),
            vec![
                ("B".to_string(), "A".to_string()),
                ("C".to_string(), "A".to_string())
            ]
        );
    }

    #[test]
    fn empty_graph_has_no_edges() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        assert!(graph.all_edges().unwrap().is_empty());
        assert!(graph.forward_links("Anything").unwrap().is_empty());
    }

    #[test]
    fn apply_replaces_only_touched_notes_edges() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[update("B", &["A", "C"]), update("D", &["A"])])
            .unwrap();

        // B now links only to C; D's edges must survive untouched.
        graph.apply(&[update("B", &["C"])], &[]).unwrap();

        assert_eq!(graph.forward_links("B").unwrap(), vec!["C".to_string()]);
        assert_eq!(graph.backlinks("A").unwrap(), vec!["D".to_string()]);
        assert_eq!(graph.backlinks("C").unwrap(), vec!["B".to_string()]);
    }

    #[test]
    fn apply_removal_drops_outgoing_but_keeps_incoming() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[update("A", &["X"]), update("B", &["A"])])
            .unwrap();

        graph.apply(&[], &["A".to_string()]).unwrap();

        // A's own outgoing edge is gone...
        assert!(graph.forward_links("A").unwrap().is_empty());
        assert!(graph.backlinks("X").unwrap().is_empty());
        // ...but B's file still says [[A]], so that edge stays (dangling).
        assert_eq!(graph.backlinks("A").unwrap(), vec!["B".to_string()]);
        assert!(!graph.stored_mtimes().unwrap().contains_key("A"));
    }

    #[test]
    fn stored_mtimes_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .apply(
                &[NoteUpdate {
                    title: "A".to_string(),
                    targets: vec![],
                    mtime: 42,
                }],
                &[],
            )
            .unwrap();
        let mtimes = graph.stored_mtimes().unwrap();
        assert_eq!(mtimes.get("A"), Some(&42));
    }
}
