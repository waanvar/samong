use std::collections::{BTreeSet, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use crate::graph::Graph;
use crate::indexer;
use crate::registry::Registry;
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
    Links {
        title: String,
        /// Also show backlinks from every registered vault
        #[arg(long)]
        all_vaults: bool,
    },
    /// List notes that no other note links to
    Orphans,
    /// List links that point at notes that do not exist
    Broken,
    /// Full-text search across the vault
    Search {
        query: String,
        /// Search a registered vault by name instead of the current directory
        #[arg(long, conflicts_with = "all_vaults")]
        vault: Option<String>,
        /// Search every registered vault
        #[arg(long)]
        all_vaults: bool,
    },
    /// Print every link-graph edge as "from -> to"
    Graph {
        /// Combine the graphs of every registered vault (nodes prefixed "vault/")
        #[arg(long)]
        all_vaults: bool,
    },
    /// List every note in the vault
    List,
    /// Watch the vault and keep the index up to date automatically
    Watch,
    /// Manage the central vault registry (~/.config/banyan)
    Vault {
        #[command(subcommand)]
        action: VaultAction,
    },
}

#[derive(Subcommand)]
enum VaultAction {
    /// Register a vault under a name usable in [[name/note]] links
    Add { name: String, path: PathBuf },
    /// List every registered vault
    List,
    /// Remove a vault from the registry (files are left untouched)
    Remove { name: String },
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

/// Path for human eyes: hide the `\\?\` verbatim prefix canonicalize adds on Windows.
fn display_path(path: &Path) -> String {
    let s = path.display().to_string();
    s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
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

/// Does a raw link target resolve to a note in another registered vault?
fn resolves_cross_vault(registry: &Registry, target: &str) -> Result<bool> {
    let Some((vault_name, title)) = vault::split_cross_vault(target) else {
        return Ok(false);
    };
    let Some(other_vault) = registry.get(vault_name)? else {
        return Ok(false);
    };
    Ok(vault::find_note(&other_vault, title)?.is_some())
}

fn cmd_broken(vault: &Path) -> Result<()> {
    indexer::reindex(vault, false)?;
    let titles: HashSet<String> = vault::list_notes(vault)?
        .into_iter()
        .map(|n| n.title)
        .collect();
    let registry = Registry::open()?;
    let mut found = false;
    let edges = {
        let graph = Graph::open(vault)?;
        graph.all_edges()?
    };
    for (from, to) in edges {
        if titles.contains(&to) || resolves_cross_vault(&registry, &to)? {
            continue;
        }
        println!("{from} -> [[{to}]]");
        found = true;
    }
    if !found {
        println!("no broken links");
    }
    Ok(())
}

fn cmd_links(vault: &Path, title: &str, all_vaults: bool) -> Result<()> {
    let (forward, back) = {
        let graph = Graph::open(vault)?;
        (graph.forward_links(title)?, graph.backlinks(title)?)
    };

    println!("forward links ({}):", forward.len());
    for target in &forward {
        println!("  -> {target}");
    }
    println!("backlinks ({}):", back.len());
    for source in &back {
        println!("  <- {source}");
    }

    if !all_vaults {
        return Ok(());
    }
    let registry = Registry::open()?;
    let Some(my_name) = registry.name_of(vault)? else {
        println!("(current vault is not registered; cross-vault backlinks unavailable)");
        return Ok(());
    };
    // Other vaults reference this note as [[my_name/title]]; each vault's own
    // graph already indexed that raw target, so one backward lookup per vault.
    let qualified = format!("{my_name}/{title}");
    let mut cross = Vec::new();
    for (other_name, other_path) in registry.list()? {
        if other_name == my_name {
            continue;
        }
        let sources = {
            let graph = Graph::open(&other_path)?;
            graph.backlinks(&qualified)?
        };
        for source in sources {
            cross.push(format!("{other_name}/{source}"));
        }
    }
    println!("cross-vault backlinks ({}):", cross.len());
    for source in cross {
        println!("  <- {source}");
    }
    Ok(())
}

fn cmd_graph(vault: &Path, all_vaults: bool) -> Result<()> {
    if !all_vaults {
        let graph = Graph::open(vault)?;
        for (from, to) in graph.all_edges()? {
            println!("{from} -> {to}");
        }
        return Ok(());
    }
    let registry = Registry::open()?;
    let vaults = registry.list()?;
    let names: HashSet<&str> = vaults.iter().map(|(n, _)| n.as_str()).collect();
    for (name, path) in &vaults {
        let edges = {
            let graph = Graph::open(path)?;
            graph.all_edges()?
        };
        for (from, to) in edges {
            // Cross-vault targets are already "vault/title"; qualify the rest.
            let to = match vault::split_cross_vault(&to) {
                Some((prefix, _)) if names.contains(prefix) => to.clone(),
                _ => format!("{name}/{to}"),
            };
            println!("{name}/{from} -> {to}");
        }
    }
    Ok(())
}

fn print_hits(hits: Vec<crate::search::SearchHit>, prefix: Option<&str>) -> bool {
    let found = !hits.is_empty();
    for hit in hits {
        match prefix {
            Some(name) => println!("{name}/{}: {}", hit.title, hit.snippet),
            None => println!("{}: {}", hit.title, hit.snippet),
        }
    }
    found
}

fn cmd_search(vault: &Path, query: &str, vault_name: Option<&str>, all_vaults: bool) -> Result<()> {
    let mut found = false;
    if all_vaults {
        let registry = Registry::open()?;
        for (name, path) in registry.list()? {
            indexer::reindex(&path, false)?;
            found |= print_hits(crate::search::query(&path, query)?, Some(&name));
        }
    } else if let Some(name) = vault_name {
        let registry = Registry::open()?;
        let path = registry
            .get(name)?
            .with_context(|| format!("vault \"{name}\" is not registered"))?;
        indexer::reindex(&path, false)?;
        found = print_hits(crate::search::query(&path, query)?, None);
    } else {
        indexer::reindex(vault, false)?;
        found = print_hits(crate::search::query(vault, query)?, None);
    }
    if !found {
        println!("no results");
    }
    Ok(())
}

fn cmd_vault(action: VaultAction) -> Result<()> {
    let registry = Registry::open()?;
    match action {
        VaultAction::Add { name, path } => {
            let canonical = registry.add(&name, &path)?;
            // Index it right away so cross-vault lookups work immediately.
            let report = indexer::reindex(&canonical, false)?;
            println!("registered \"{name}\" at {}", display_path(&canonical));
            println!("{report}");
        }
        VaultAction::List => {
            let vaults = registry.list()?;
            if vaults.is_empty() {
                println!("no vaults registered");
            }
            for (name, path) in vaults {
                println!("{name}\t{}", display_path(&path));
            }
        }
        VaultAction::Remove { name } => {
            if registry.remove(&name)? {
                println!("removed \"{name}\" from the registry");
            } else {
                bail!("vault \"{name}\" is not registered");
            }
        }
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
        Command::Links { title, all_vaults } => cmd_links(&vault, &title, all_vaults)?,
        Command::Orphans => cmd_orphans(&vault)?,
        Command::Broken => cmd_broken(&vault)?,
        Command::Search {
            query,
            vault: vault_name,
            all_vaults,
        } => cmd_search(&vault, &query, vault_name.as_deref(), all_vaults)?,
        Command::Graph { all_vaults } => cmd_graph(&vault, all_vaults)?,
        Command::List => {
            for note in vault::list_notes(&vault)? {
                println!("{}", note.title);
            }
        }
        Command::Watch => watch::run(&vault)?,
        Command::Vault { action } => cmd_vault(action)?,
    }

    Ok(())
}
