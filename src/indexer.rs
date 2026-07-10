use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};

use crate::graph::{Graph, NoteUpdate};
use crate::search;
use crate::vault::{self, Note};

pub struct ReindexReport {
    pub indexed: usize,
    pub removed: usize,
    pub full: bool,
}

impl std::fmt::Display for ReindexReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.full {
            write!(f, "reindexed {} note(s) (full)", self.indexed)
        } else {
            write!(
                f,
                "reindexed {} note(s), removed {}",
                self.indexed, self.removed
            )
        }
    }
}

fn file_mtime(path: &Path) -> Result<u64> {
    let modified = fs::metadata(path)
        .and_then(|m| m.modified())
        .with_context(|| format!("reading mtime of {}", path.display()))?;
    Ok(modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64)
}

fn load_update(note: &Note) -> Result<(NoteUpdate, String)> {
    let content = fs::read_to_string(&note.path)
        .with_context(|| format!("reading note {}", note.path.display()))?;
    let targets = vault::parse_wikilinks(&content)
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok((
        NoteUpdate {
            title: note.title.clone(),
            targets,
            mtime: file_mtime(&note.path)?,
        },
        content,
    ))
}

/// Bring both indexes (link graph + full-text) in sync with the .md files on disk.
///
/// Incremental mode compares each file's mtime against the value recorded in
/// redb at last index time and only touches notes that were added, changed,
/// or deleted. `full` rebuilds everything from scratch.
pub fn reindex(vault: &Path, full: bool) -> Result<ReindexReport> {
    let notes = vault::list_notes(vault)?;
    let graph = Graph::open(vault)?;

    if full {
        let mut updates = Vec::with_capacity(notes.len());
        let mut bodies = Vec::with_capacity(notes.len());
        for note in &notes {
            let (update, content) = load_update(note)?;
            bodies.push((note.title.clone(), content));
            updates.push(update);
        }
        graph.rebuild(&updates)?;
        search::rebuild(vault, &bodies)?;
        return Ok(ReindexReport {
            indexed: notes.len(),
            removed: 0,
            full: true,
        });
    }

    let mut stored = graph.stored_mtimes()?;
    let mut updates = Vec::new();
    let mut bodies = Vec::new();
    for note in &notes {
        let known = stored.remove(&note.title);
        if known == Some(file_mtime(&note.path)?) {
            continue;
        }
        let (update, content) = load_update(note)?;
        bodies.push((note.title.clone(), content));
        updates.push(update);
    }
    // Whatever is left in `stored` no longer exists on disk.
    let removals: Vec<String> = stored.into_keys().collect();

    let report = ReindexReport {
        indexed: updates.len(),
        removed: removals.len(),
        full: false,
    };
    if !updates.is_empty() || !removals.is_empty() {
        graph.apply(&updates, &removals)?;
        search::apply(vault, &bodies, &removals)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_reindex_only_touches_changed_notes() {
        let dir = tempfile::tempdir().unwrap();
        vault::create_note(dir.path(), "A").unwrap();
        vault::create_note(dir.path(), "B").unwrap();

        let report = reindex(dir.path(), false).unwrap();
        assert_eq!(report.indexed, 2);

        // Nothing changed: nothing to do.
        let report = reindex(dir.path(), false).unwrap();
        assert_eq!(report.indexed, 0);
        assert_eq!(report.removed, 0);

        // Touch one file only.
        fs::write(dir.path().join("A.md"), "# A\n\nnew [[B]] link\n").unwrap();
        let report = reindex(dir.path(), false).unwrap();
        assert_eq!(report.indexed, 1);
        assert_eq!(report.removed, 0);

        let graph = Graph::open(dir.path()).unwrap();
        assert_eq!(graph.backlinks("B").unwrap(), vec!["A".to_string()]);
    }

    #[test]
    fn incremental_reindex_detects_deleted_notes() {
        let dir = tempfile::tempdir().unwrap();
        vault::create_note(dir.path(), "A").unwrap();
        vault::create_note(dir.path(), "B").unwrap();
        reindex(dir.path(), false).unwrap();

        fs::remove_file(dir.path().join("B.md")).unwrap();
        let report = reindex(dir.path(), false).unwrap();
        assert_eq!(report.indexed, 0);
        assert_eq!(report.removed, 1);

        assert!(search::query(dir.path(), "B").unwrap().is_empty());
    }

    #[test]
    fn reindex_reads_notes_in_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("projects");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("Deep Note.md"), "# Deep Note\n\nlinks [[A]]\n").unwrap();
        vault::create_note(dir.path(), "A").unwrap();

        reindex(dir.path(), false).unwrap();

        let graph = Graph::open(dir.path()).unwrap();
        assert_eq!(graph.backlinks("A").unwrap(), vec!["Deep Note".to_string()]);
    }
}
