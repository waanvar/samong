use std::collections::{BTreeSet, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use crate::graph::Graph;
use crate::indexer;
use crate::vault;
use crate::watch;

#[derive(Parser)]
#[command(
    name = "banyan",
    version,
    about = "A local-first, Obsidian-compatible knowledge base"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new, empty note and index it
    New { title: String },
    /// Open a note in $EDITOR, then reindex it
    Edit { title: String },
    /// Delete a note and report backlinks that will dangle
    Delete { title: String },
    /// Rename a note and rewrite every [[wikilink]] pointing at it
    Rename { old: String, new: String },
    /// Sync the link graph and full-text index with the .md files on disk
    Reindex {
        /// Rebuild everything from scratch instead of only changed files
        #[arg(long)]
        full: bool,
    },
    /// Show forward links and backlinks for a note
    Links { title: String },
    /// List notes that no other note links to
    Orphans,
    /// List links that point at notes that do not exist
    Broken,
    /// Full-text search across the vault
    Search { query: String },
    /// Print every link-graph edge as "from -> to"
    Graph,
    /// List every note in the vault
    List,
    /// Watch the vault and keep the index up to date automatically
    Watch,
}

/// The vault is always the current working directory.
fn vault_root() -> Result<PathBuf> {
    Ok(env::current_dir()?)
}

/// Resolve the editor to launch: $EDITOR (split on whitespace so values like
/// "code --wait" work), falling back to a sensible platform default.
fn editor_command() -> (String, Vec<String>) {
    if let Ok(editor) = env::var("EDITOR") {
        let mut parts = editor.split_whitespace().map(str::to_string);
        if let Some(program) = parts.next() {
            return (program, parts.collect());
        }
    }
    if cfg!(windows) {
        ("notepad".to_string(), Vec::new())
    } else {
        ("vi".to_string(), Vec::new())
    }
}

fn require_note(vault: &Path, title: &str) -> Result<vault::Note> {
    vault::find_note(vault, title)?.with_context(|| format!("note \"{title}\" does not exist"))
}

fn cmd_edit(vault: &Path, title: &str) -> Result<()> {
    let note = require_note(vault, title)?;
    let (program, args) = editor_command();
    let status = process::Command::new(&program)
        .args(&args)
        .arg(&note.path)
        .status()
        .with_context(|| format!("launching editor \"{program}\""))?;
    if !status.success() {
        bail!("editor \"{program}\" exited with {status}");
    }
    let report = indexer::reindex(vault, false)?;
    println!("{report}");
    Ok(())
}

fn cmd_delete(vault: &Path, title: &str) -> Result<()> {
    let note = require_note(vault, title)?;
    indexer::reindex(vault, false)?;

    // Scoped so the db handle is released before the reindex below reopens it.
    let dangling: Vec<String> = {
        let graph = Graph::open(vault)?;
        graph
            .backlinks(title)?
            .into_iter()
            .filter(|source| source != title)
            .collect()
    };

    fs::remove_file(&note.path).with_context(|| format!("deleting {}", note.path.display()))?;
    indexer::reindex(vault, false)?;

    println!("deleted \"{title}\"");
    if !dangling.is_empty() {
        println!("warning: {} note(s) still link to it:", dangling.len());
        for source in dangling {
            println!("  {source} -> [[{title}]]");
        }
    }
    Ok(())
}

fn cmd_rename(vault: &Path, old: &str, new: &str) -> Result<()> {
    let note = require_note(vault, old)?;
    if vault::find_note(vault, new)?.is_some() {
        bail!("note \"{new}\" already exists");
    }
    indexer::reindex(vault, false)?;

    // Rewrite [[old]] in every note that links here (including self-links).
    // Scoped so the db handle is released before the reindex below reopens it.
    let sources: BTreeSet<String> = {
        let graph = Graph::open(vault)?;
        graph.backlinks(old)?.into_iter().collect()
    };
    let mut rewritten_links = 0;
    let mut rewritten_notes = 0;
    for source in &sources {
        let Some(source_note) = vault::find_note(vault, source)? else {
            continue; // dangling backlink from an already-deleted note
        };
        let content = fs::read_to_string(&source_note.path)
            .with_context(|| format!("reading {}", source_note.path.display()))?;
        let (updated, count) = vault::rewrite_wikilinks(&content, old, new);
        if count > 0 {
            fs::write(&source_note.path, updated)
                .with_context(|| format!("writing {}", source_note.path.display()))?;
            rewritten_links += count;
            rewritten_notes += 1;
        }
    }

    // Move the file itself (keeping it in the same directory), then resync.
    let new_path = note.path.with_file_name(format!("{new}.md"));
    fs::rename(&note.path, &new_path)
        .with_context(|| format!("renaming {} -> {}", note.path.display(), new_path.display()))?;
    indexer::reindex(vault, false)?;

    println!("renamed \"{old}\" -> \"{new}\"");
    println!("updated {rewritten_links} link(s) in {rewritten_notes} note(s)");
    Ok(())
}

fn cmd_orphans(vault: &Path) -> Result<()> {
    indexer::reindex(vault, false)?;
    let graph = Graph::open(vault)?;
    let mut found = false;
    for note in vault::list_notes(vault)? {
        if graph.backlinks(&note.title)?.is_empty() {
            println!("{}", note.title);
            found = true;
        }
    }
    if !found {
        println!("no orphans");
    }
    Ok(())
}

fn cmd_broken(vault: &Path) -> Result<()> {
    indexer::reindex(vault, false)?;
    let titles: HashSet<String> = vault::list_notes(vault)?
        .into_iter()
        .map(|n| n.title)
        .collect();
    let graph = Graph::open(vault)?;
    let mut found = false;
    for (from, to) in graph.all_edges()? {
        if !titles.contains(&to) {
            println!("{from} -> [[{to}]]");
            found = true;
        }
    }
    if !found {
        println!("no broken links");
    }
    Ok(())
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let vault = vault_root()?;

    match cli.command {
        Command::New { title } => {
            vault::create_note(&vault, &title)?;
            indexer::reindex(&vault, false)?;
            println!("created \"{title}\"");
        }
        Command::Edit { title } => cmd_edit(&vault, &title)?,
        Command::Delete { title } => cmd_delete(&vault, &title)?,
        Command::Rename { old, new } => cmd_rename(&vault, &old, &new)?,
        Command::Reindex { full } => {
            let report = indexer::reindex(&vault, full)?;
            println!("{report}");
            println!("reindex complete");
        }
        Command::Links { title } => {
            let graph = Graph::open(&vault)?;
            let forward = graph.forward_links(&title)?;
            let back = graph.backlinks(&title)?;

            println!("forward links ({}):", forward.len());
            for target in &forward {
                println!("  -> {target}");
            }
            println!("backlinks ({}):", back.len());
            for source in &back {
                println!("  <- {source}");
            }
        }
        Command::Orphans => cmd_orphans(&vault)?,
        Command::Broken => cmd_broken(&vault)?,
        Command::Search { query } => {
            let hits = crate::search::query(&vault, &query)?;
            if hits.is_empty() {
                println!("no results");
            }
            for hit in hits {
                println!("{}: {}", hit.title, hit.snippet);
            }
        }
        Command::Graph => {
            let graph = Graph::open(&vault)?;
            for (from, to) in graph.all_edges()? {
                println!("{from} -> {to}");
            }
        }
        Command::List => {
            for note in vault::list_notes(&vault)? {
                println!("{}", note.title);
            }
        }
        Command::Watch => watch::run(&vault)?,
    }

    Ok(())
}
