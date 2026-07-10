use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{Event, RecursiveMode, Watcher};

use crate::indexer;
use crate::vault::BRAIN_DIR;

const DEBOUNCE: Duration = Duration::from_millis(300);

/// A filesystem event matters only if it touches a .md file outside `.brain/`
/// — our own index writes must never re-trigger a reindex.
fn is_relevant(event: &Event, brain_dir: &Path) -> bool {
    event.paths.iter().any(|path| {
        path.extension().and_then(|e| e.to_str()) == Some("md") && !path.starts_with(brain_dir)
    })
}

/// Watch the vault and incrementally reindex whenever notes change.
/// Runs until the process is interrupted (Ctrl+C).
pub fn run(vault: &Path) -> Result<()> {
    let brain_dir = vault.join(BRAIN_DIR);

    // Start from a consistent state.
    let report = indexer::reindex(vault, false)?;
    println!("{report}");

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(tx).context("creating filesystem watcher")?;
    watcher
        .watch(vault, RecursiveMode::Recursive)
        .with_context(|| format!("watching {}", vault.display()))?;
    println!("watching {} (Ctrl+C to stop)", vault.display());

    loop {
        // Block until something relevant happens.
        let Ok(event) = rx.recv() else {
            return Ok(()); // watcher dropped; nothing left to do
        };
        let mut pending = matches!(&event, Ok(e) if is_relevant(e, &brain_dir));

        // Editors often emit bursts of events per save — absorb them briefly.
        let deadline = Instant::now() + DEBOUNCE;
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(remaining) {
                Ok(Ok(e)) if is_relevant(&e, &brain_dir) => pending = true,
                Ok(_) => {}
                Err(_) => break,
            }
        }

        if !pending {
            continue;
        }
        match indexer::reindex(vault, false) {
            Ok(report) if report.indexed > 0 || report.removed > 0 => println!("{report}"),
            Ok(_) => {}
            // A save can race the scan (half-written file); the next event retries.
            Err(err) => eprintln!("reindex failed: {err:#}"),
        }
    }
}
