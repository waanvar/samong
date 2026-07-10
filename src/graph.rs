use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use redb::{Database, MultimapTableDefinition, ReadableMultimapTable};

use crate::vault::BRAIN_DIR;

const FORWARD: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("forward");
const BACKWARD: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("backward");

/// Link graph backed by redb: forward links (raw wikilink targets, which may or may not
/// correspond to a real note) and backlinks (their inverse), rebuilt in full on every reindex.
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

    /// Replace the entire graph with the given `(from_title, to_target)` edges.
    pub fn rebuild(&self, edges: &[(String, String)]) -> Result<()> {
        let txn = self
            .db
            .begin_write()
            .context("beginning write transaction")?;
        txn.delete_multimap_table(FORWARD)
            .context("clearing forward table")?;
        txn.delete_multimap_table(BACKWARD)
            .context("clearing backward table")?;
        {
            let mut forward = txn.open_multimap_table(FORWARD)?;
            let mut backward = txn.open_multimap_table(BACKWARD)?;
            for (from, to) in edges {
                forward.insert(from.as_str(), to.as_str())?;
                backward.insert(to.as_str(), from.as_str())?;
            }
        }
        txn.commit().context("committing graph rebuild")?;
        Ok(())
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

    #[test]
    fn rebuild_and_query_links() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[
                ("B".to_string(), "A".to_string()),
                ("C".to_string(), "A".to_string()),
            ])
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
        graph
            .rebuild(&[("X".to_string(), "Y".to_string())])
            .unwrap();
        assert_eq!(graph.forward_links("X").unwrap(), vec!["Y".to_string()]);

        graph.rebuild(&[]).unwrap();
        assert!(graph.forward_links("X").unwrap().is_empty());
        assert!(graph.backlinks("Y").unwrap().is_empty());
    }

    #[test]
    fn all_edges_returns_every_pair() {
        let dir = tempfile::tempdir().unwrap();
        let graph = Graph::open(dir.path()).unwrap();
        graph
            .rebuild(&[
                ("B".to_string(), "A".to_string()),
                ("C".to_string(), "A".to_string()),
            ])
            .unwrap();
        let mut edges = graph.all_edges().unwrap();
        edges.sort();
        assert_eq!(
            edges,
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
}
