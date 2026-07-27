use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};

use crate::graph::{Graph, NoteUpdate};
use crate::scope::Scope;
use crate::search::{self, IndexedNote};
use crate::vault::{self, Note};

/// Bump whenever the tantivy schema, the tokenizer, or the note identity scheme
/// changes; vaults indexed with an older version are rebuilt in full
/// automatically.
/// 1 = default tokenizer (pre-versioning), 2 = thai_mixed tokenizer,
/// 3 = notes keyed by vault-relative path instead of title.
pub const INDEX_VERSION: u64 = 3;

pub struct ReindexReport {
    pub indexed: usize,
    pub removed: usize,
    pub full: bool,
    /// The rebuild was forced by an index-format change, not requested.
    pub upgraded: bool,
    /// Files whose mtime moved but whose bytes did not, so no reindexing was
    /// needed. A `git checkout` or a fresh clone produces a pile of these.
    pub untouched: usize,
    /// `scope.include` roots that are declared but absent on this machine.
    /// Surfaced on every report because the alternative — quietly indexing a few
    /// hundred notes fewer than the config asks for — reads as "search is
    /// broken" to whoever forgot to install dependencies.
    pub missing_includes: Vec<String>,
}

impl std::fmt::Display for ReindexReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.full {
            write!(f, "reindexed {} note(s) (full)", self.indexed)?;
        } else {
            write!(
                f,
                "reindexed {} note(s), removed {}",
                self.indexed, self.removed
            )?;
        }
        if self.untouched > 0 {
            write!(f, " ({} unchanged despite new mtime)", self.untouched)?;
        }
        if self.upgraded {
            write!(f, " [index format changed; rebuilt automatically]")?;
        }
        if !self.missing_includes.is_empty() {
            write!(
                f,
                "\nwarning: scope.include director{} not found on this machine: {} \
                 (reference notes from there are missing; install dependencies?)",
                if self.missing_includes.len() == 1 {
                    "y"
                } else {
                    "ies"
                },
                self.missing_includes.join(", ")
            )?;
        }
        Ok(())
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

/// Content identity. mtime alone cannot serve: a checkout, a clone, or a copy
/// between machines rewrites mtimes without changing a single byte.
fn content_hash(content: &str) -> String {
    blake3::hash(content.as_bytes()).to_hex().to_string()
}

/// Read a note and derive everything the indexes need from it.
fn load_update(note: &Note, mtime: u64) -> Result<(NoteUpdate, String)> {
    let content = fs::read_to_string(&note.path)
        .with_context(|| format!("reading note {}", note.path.display()))?;
    let targets = vault::parse_wikilinks(&content)
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok((
        NoteUpdate {
            key: note.key.clone(),
            title: note.title.clone(),
            targets,
            mtime,
            hash: content_hash(&content),
        },
        content,
    ))
}

fn indexed(update: &NoteUpdate, body: String) -> IndexedNote {
    IndexedNote {
        key: update.key.clone(),
        title: update.title.clone(),
        body,
    }
}

/// Bring both indexes (link graph + full-text) in sync with the .md files on
/// disk, using the vault's own scope rules to decide what a note is.
pub fn reindex(vault: &Path, full: bool) -> Result<ReindexReport> {
    reindex_in(&Scope::load(vault)?, full)
}

/// Same as [`reindex`], for callers that already hold the vault's [`Scope`].
///
/// Incremental mode compares each file's mtime against the value recorded at
/// last index time; when the mtime moved it compares content hashes before
/// doing any real work. `full` rebuilds everything from scratch.
pub fn reindex_in(scope: &Scope, full: bool) -> Result<ReindexReport> {
    let vault = scope.root();
    let notes = vault::list_notes_in(scope)?;
    let graph = Graph::open(vault)?;

    // A schema/tokenizer/identity change invalidates the whole index.
    let upgraded = graph.index_version()? != Some(INDEX_VERSION);
    let full = full || upgraded;

    if full {
        let mut updates = Vec::with_capacity(notes.len());
        let mut bodies = Vec::with_capacity(notes.len());
        for note in &notes {
            let (update, content) = load_update(note, file_mtime(&note.path)?)?;
            bodies.push(indexed(&update, content));
            updates.push(update);
        }
        graph.rebuild(&updates)?;
        search::rebuild(vault, &bodies)?;
        graph.set_index_version(INDEX_VERSION)?;
        return Ok(ReindexReport {
            indexed: notes.len(),
            removed: 0,
            full: true,
            upgraded,
            untouched: 0,
            missing_includes: scope.missing_include_roots(),
        });
    }

    let mut stored = graph.stored_files()?;
    let mut updates = Vec::new();
    let mut bodies = Vec::new();
    let mut untouched = 0;
    for note in &notes {
        let known = stored.remove(&note.key);
        let mtime = file_mtime(&note.path)?;
        if known.as_ref().is_some_and(|state| state.mtime == mtime) {
            continue; // not even touched
        }
        let (update, content) = load_update(note, mtime)?;
        if known.is_some_and(|state| state.hash == update.hash) {
            // Touched but identical: record the new mtime so the next run stops
            // re-reading it, and leave the search index alone.
            untouched += 1;
            updates.push(update);
            continue;
        }
        bodies.push(indexed(&update, content));
        updates.push(update);
    }
    // Whatever is left in `stored` no longer exists on disk.
    let removals: Vec<String> = stored.into_keys().collect();

    let report = ReindexReport {
        // Notes whose content actually changed; mtime-only touches are counted
        // separately so the numbers add up for a human reading them.
        indexed: bodies.len(),
        removed: removals.len(),
        full: false,
        upgraded: false,
        untouched,
        missing_includes: scope.missing_include_roots(),
    };
    if !updates.is_empty() || !removals.is_empty() {
        graph.apply(&updates, &removals)?;
    }
    if !bodies.is_empty() || !removals.is_empty() {
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
        assert_eq!(graph.backlinks("B").unwrap(), vec!["A.md".to_string()]);
    }

    /// A `git checkout` (or clone, or file copy) rewrites mtimes without
    /// touching a byte. mtime alone would reindex the whole vault; the content
    /// hash catches it.
    #[test]
    fn touched_but_identical_files_are_not_reindexed() {
        let dir = tempfile::tempdir().unwrap();
        vault::create_note(dir.path(), "A").unwrap();
        reindex(dir.path(), false).unwrap();

        let path = dir.path().join("A.md");
        let content = fs::read_to_string(&path).unwrap();
        // Rewrite the same bytes, which moves the mtime.
        fs::write(&path, &content).unwrap();

        let report = reindex(dir.path(), false).unwrap();
        assert_eq!(report.indexed, 0, "content did not change");
        assert_eq!(
            report.untouched, 1,
            "the new mtime was noticed and recorded"
        );

        // The refreshed mtime is stored, so the next run has nothing to read.
        let report = reindex(dir.path(), false).unwrap();
        assert_eq!(report.indexed, 0);
        assert_eq!(report.untouched, 0);
    }

    /// The reported regression: a repo full of `README.md` files used to collapse
    /// into one note, and to reindex itself forever because the shared key's
    /// mtime never matched.
    #[test]
    fn duplicate_titles_are_indexed_separately_and_settle() {
        let dir = tempfile::tempdir().unwrap();
        for sub in ["", "docs", "api"] {
            let dir_path = dir.path().join(sub);
            fs::create_dir_all(&dir_path).unwrap();
            fs::write(
                dir_path.join("README.md"),
                format!("# README\n\nin {sub:?} linking [[Hub]]\n"),
            )
            .unwrap();
        }

        let report = reindex(dir.path(), false).unwrap();
        assert_eq!(report.indexed, 3, "each file is its own note");

        // Second run must be a no-op: the old title-keyed index never settled.
        let report = reindex(dir.path(), false).unwrap();
        assert_eq!(report.indexed, 0);
        assert_eq!(report.removed, 0);
        assert_eq!(report.untouched, 0);

        let graph = Graph::open(dir.path()).unwrap();
        assert_eq!(
            graph.backlinks("Hub").unwrap(),
            vec![
                "README.md".to_string(),
                "api/README.md".to_string(),
                "docs/README.md".to_string()
            ]
        );
        assert_eq!(graph.duplicate_titles().unwrap().len(), 1);
        // All three are searchable, not just the last one indexed.
        assert_eq!(search::query(dir.path(), "linking").unwrap().len(), 3);
    }

    #[test]
    fn out_of_scope_files_are_never_indexed() {
        let dir = tempfile::tempdir().unwrap();
        vault::create_note(dir.path(), "Real").unwrap();
        let dep = dir.path().join("node_modules").join("left-pad");
        fs::create_dir_all(&dep).unwrap();
        fs::write(dep.join("README.md"), "# left-pad\n\nsome dependency\n").unwrap();
        fs::write(dir.path().join(".gitignore"), "generated.md\n").unwrap();
        fs::write(dir.path().join("generated.md"), "# generated\n").unwrap();

        let report = reindex(dir.path(), false).unwrap();
        assert_eq!(report.indexed, 1);
        assert!(search::query(dir.path(), "dependency").unwrap().is_empty());
        assert!(search::query(dir.path(), "generated").unwrap().is_empty());
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
        // Backlink sources come back as keys: the full path inside the vault.
        assert_eq!(
            graph.backlinks("A").unwrap(),
            vec!["projects/Deep Note.md".to_string()]
        );
    }
}
