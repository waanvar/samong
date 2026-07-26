use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{Event, RecursiveMode, Watcher};

use crate::indexer;
use crate::scope::Scope;

const DEBOUNCE: Duration = Duration::from_millis(300);

/// A filesystem event matters only if it touches a file the vault's scope would
/// accept as a note — our own index writes, `node_modules`, and anything
/// gitignored must never re-trigger a reindex.
fn is_relevant(event: &Event, scope: &Scope) -> bool {
    event.paths.iter().any(|path| scope.may_include(path))
}

/// Point the watcher at exactly the directories in scope.
///
/// Recursively watching the vault root would hand the OS every dependency
/// directory too: on Linux one `node_modules` can exhaust `max_user_watches`
/// and make watch mode fail outright, and a single `npm install` would wake the
/// indexer thousands of times.
fn watch_targets(watcher: &mut dyn Watcher, targets: &[PathBuf]) -> Result<()> {
    for (i, target) in targets.iter().enumerate() {
        // The notes root is watched non-recursively; each in-scope subdirectory
        // brings its own recursive watch. New top-level directories are picked
        // up when the watch set is refreshed after a reindex.
        let mode = if i == 0 {
            RecursiveMode::NonRecursive
        } else {
            RecursiveMode::Recursive
        };
        watcher
            .watch(target, mode)
            .with_context(|| format!("watching {}", target.display()))?;
    }
    Ok(())
}

fn unwatch_all(watcher: &mut dyn Watcher, targets: &[PathBuf]) {
    for target in targets {
        // Best effort: a directory may already be gone, which is why we rewatch.
        let _ = watcher.unwatch(target);
    }
}

/// Watch the vault and incrementally reindex whenever notes change.
/// Runs until the process is interrupted (Ctrl+C).
pub fn run(vault: &Path) -> Result<()> {
    let mut scope = Scope::load(vault)?;

    // Start from a consistent state.
    let report = indexer::reindex_in(&scope, false)?;
    println!("{report}");

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(tx).context("creating filesystem watcher")?;
    let mut targets = scope.watch_targets()?;
    watch_targets(&mut watcher, &targets)?;
    println!(
        "watching {} ({} dir(s) in scope, Ctrl+C to stop)",
        scope.notes_root().display(),
        targets.len()
    );

    loop {
        // Block until something relevant happens.
        let Ok(event) = rx.recv() else {
            return Ok(()); // watcher dropped; nothing left to do
        };
        let mut pending = matches!(&event, Ok(e) if is_relevant(e, &scope));

        // Editors often emit bursts of events per save — absorb them briefly.
        let deadline = Instant::now() + DEBOUNCE;
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(remaining) {
                Ok(Ok(e)) if is_relevant(&e, &scope) => pending = true,
                Ok(_) => {}
                Err(_) => break,
            }
        }

        if !pending {
            continue;
        }

        // The scope rules themselves live in the vault, so a change to them
        // changes what counts as a note. Reload before scanning.
        scope = Scope::load(vault)?;
        match indexer::reindex_in(&scope, false) {
            Ok(report) if report.indexed > 0 || report.removed > 0 => {
                println!("{report}");
                // Notes may have appeared in a directory nobody was watching.
                let refreshed = scope.watch_targets()?;
                if refreshed != targets {
                    unwatch_all(&mut watcher, &targets);
                    watch_targets(&mut watcher, &refreshed)?;
                    targets = refreshed;
                }
            }
            Ok(_) => {}
            // A save can race the scan (half-written file); the next event retries.
            Err(err) => eprintln!("reindex failed: {err:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, EventKind};

    fn event(paths: Vec<PathBuf>) -> Event {
        Event {
            kind: EventKind::Create(CreateKind::File),
            paths,
            attrs: Default::default(),
        }
    }

    #[test]
    fn only_in_scope_notes_trigger_a_reindex() {
        let dir = tempfile::tempdir().unwrap();
        let scope = Scope::load(dir.path()).unwrap();

        assert!(is_relevant(
            &event(vec![dir.path().join("Note.md")]),
            &scope
        ));
        assert!(!is_relevant(
            &event(vec![dir.path().join("Note.txt")]),
            &scope
        ));
        // Our own index writes must not feed back into a reindex loop.
        assert!(!is_relevant(
            &event(vec![dir.path().join(crate::vault::BRAIN_DIR).join("x.md")]),
            &scope
        ));
        // An `npm install` churning through dependency READMEs is not news.
        assert!(!is_relevant(
            &event(vec![dir.path().join("node_modules/dep/README.md")]),
            &scope
        ));
        // A burst that includes one real note still counts.
        assert!(is_relevant(
            &event(vec![
                dir.path().join("node_modules/dep/README.md"),
                dir.path().join("Real.md"),
            ]),
            &scope
        ));
    }
}
