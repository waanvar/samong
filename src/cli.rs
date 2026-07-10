use std::env;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::graph::Graph;
use crate::vault;

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
    /// Create a new, empty note and reindex the vault
    New { title: String },
    /// Rebuild the link graph and full-text index from every .md file in the vault
    Reindex,
    /// Show forward links and backlinks for a note
    Links { title: String },
    /// Full-text search across the vault
    Search { query: String },
    /// Print every link-graph edge as "from -> to"
    Graph,
    /// List every note in the vault
    List,
}

/// The vault is always the current working directory in Phase 0.
fn vault_root() -> Result<PathBuf> {
    Ok(env::current_dir()?)
}

/// Rebuild both the link graph and the full-text index from the notes currently on disk.
fn reindex(vault: &std::path::Path) -> Result<()> {
    let notes = vault::list_notes(vault)?;

    let mut edges = Vec::new();
    let mut bodies = Vec::new();
    for note in &notes {
        let content = vault::read_note(vault, &note.title)?;
        for link in vault::parse_wikilinks(&content) {
            edges.push((note.title.clone(), link.target));
        }
        bodies.push((note.title.clone(), content));
    }

    Graph::open(vault)?.rebuild(&edges)?;
    crate::search::rebuild(vault, &bodies)?;
    Ok(())
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let vault = vault_root()?;

    match cli.command {
        Command::New { title } => {
            vault::create_note(&vault, &title)?;
            reindex(&vault)?;
            println!("created \"{title}\"");
        }
        Command::Reindex => {
            reindex(&vault)?;
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
    }

    Ok(())
}
