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
use crate::scope::Scope;
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
        /// Maximum results to show
        #[arg(long, default_value_t = crate::search::DEFAULT_LIMIT)]
        limit: usize,
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
    /// Report on the vault: what is in scope, what was skipped, and any
    /// ambiguous note titles
    Doctor,
    /// Update banyan to the latest GitHub release
    Update {
        /// Only report whether an update is available; don't install it
        #[arg(long)]
        check: bool,
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
        let sources = graph
            .backlinks(title)?
            .into_iter()
            .filter(|source_key| *source_key != note.key) // a self-link is not dangling
            .collect();
        crate::ops::keys_to_titles(sources)
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
    let source_keys: BTreeSet<String> = {
        let graph = Graph::open(vault)?;
        graph.backlinks(old)?.into_iter().collect()
    };
    let mut rewritten_links = 0;
    let mut rewritten_notes = 0;
    for source_key in &source_keys {
        // Keys are vault-relative paths, so the file is addressed directly —
        // no title lookup, and no ambiguity when several notes share a title.
        let source_path = vault.join(source_key);
        if !source_path.is_file() {
            continue; // dangling backlink from an already-deleted note
        }
        let content = fs::read_to_string(&source_path)
            .with_context(|| format!("reading {}", source_path.display()))?;
        let (updated, count) = vault::rewrite_wikilinks(&content, old, new);
        if count > 0 {
            fs::write(&source_path, updated)
                .with_context(|| format!("writing {}", source_path.display()))?;
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
    for (from_key, to) in edges {
        if titles.contains(&to) || resolves_cross_vault(&registry, &to)? {
            continue;
        }
        let from = crate::graph::title_from_key(&from_key).unwrap_or(from_key);
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
        (
            // A title can name more than one file; show the links of all of them.
            graph.forward_links_for_title(title)?,
            crate::ops::keys_to_titles(graph.backlinks(title)?),
        )
    };

    let sharing = vault::find_notes(vault, title)?;
    if sharing.len() > 1 {
        println!(
            "note: {} files share the title \"{title}\" ({}) — their links are merged below",
            sharing.len(),
            sharing
                .iter()
                .map(|n| n.key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

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
    let cross = crate::ops::cross_vault_backlinks(&registry, &my_name, title)?;
    println!("cross-vault backlinks ({}):", cross.len());
    for source in cross {
        println!("  <- {source}");
    }
    Ok(())
}

/// Edges are stored as `(note key, raw target)`; the target names a title, so
/// the source is shown as a title too and the dump stays in one namespace.
fn edge_source_title(key: String) -> String {
    crate::graph::title_from_key(&key).unwrap_or(key)
}

fn cmd_graph(vault: &Path, all_vaults: bool) -> Result<()> {
    if !all_vaults {
        let graph = Graph::open(vault)?;
        for (from, to) in graph.all_edges()? {
            println!("{} -> {to}", edge_source_title(from));
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
            println!("{name}/{} -> {to}", edge_source_title(from));
        }
    }
    Ok(())
}

/// Results are labelled with the note's path, not its bare title: search is
/// exactly where two files called `README` have to be told apart, and the path
/// contains the title anyway.
fn print_hits(hits: Vec<crate::search::SearchHit>, prefix: Option<&str>) -> bool {
    let found = !hits.is_empty();
    for hit in hits {
        match prefix {
            Some(name) => println!("{name}/{}: {}", hit.key, hit.snippet),
            None => println!("{}: {}", hit.key, hit.snippet),
        }
    }
    found
}

fn cmd_search(
    vault: &Path,
    query: &str,
    vault_name: Option<&str>,
    all_vaults: bool,
    limit: usize,
) -> Result<()> {
    let options = crate::search::SearchOptions::with_limit(limit);
    let mut found = false;
    if all_vaults {
        let registry = Registry::open()?;
        for (name, path) in registry.list()? {
            indexer::reindex(&path, false)?;
            found |= print_hits(
                crate::search::query_with(&path, query, &options)?,
                Some(&name),
            );
        }
    } else if let Some(name) = vault_name {
        let registry = Registry::open()?;
        let path = registry
            .get(name)?
            .with_context(|| format!("vault \"{name}\" is not registered"))?;
        indexer::reindex(&path, false)?;
        found = print_hits(crate::search::query_with(&path, query, &options)?, None);
    } else {
        indexer::reindex(vault, false)?;
        found = print_hits(crate::search::query_with(vault, query, &options)?, None);
    }
    if !found {
        println!("no results");
    }
    Ok(())
}

/// Print what the scope rules let in and kept out.
///
/// Registering a vault is the moment a wrong scope gets baked in, so the
/// numbers are shown then and there rather than waiting for someone to wonder
/// why search returns a dependency's README. This never asks a question:
/// pointing `vault add` at a repo root is a perfectly good thing to do, and the
/// default rules already handle it.
fn print_scope_summary(scope: &Scope) -> Result<()> {
    let audit = scope.audit()?;
    let about = if audit.truncated { "at least " } else { "" };

    println!("{} note(s) in scope", audit.included);
    if audit.skipped > 0 {
        let breakdown = audit
            .skipped_by_dir
            .iter()
            .take(3)
            .map(|(dir, count)| format!("{dir} {count}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "skipped {about}{} .md file(s) not tracked as notes ({breakdown})",
            audit.skipped
        );
    }
    if audit.included == 0 {
        println!(
            "warning: no notes found — check scope.notes_dir in {}, or whether .gitignore excludes them",
            crate::scope::CONFIG_FILE
        );
    }
    Ok(())
}

fn cmd_doctor(vault: &Path) -> Result<()> {
    let scope = Scope::load(vault)?;
    println!("vault: {}", display_path(scope.root()));
    if let Some(name) = &scope.config().vault.name {
        println!("name (from {}): {name}", crate::scope::CONFIG_FILE);
    }
    if scope.notes_root() != scope.root() {
        println!("notes dir: {}", display_path(scope.notes_root()));
    }
    println!(
        "gitignore: {}",
        if scope.config().scope.follow_gitignore {
            "respected"
        } else {
            "disabled in config"
        }
    );
    print_scope_summary(&scope)?;

    let report = indexer::reindex_in(&scope, false)?;
    println!("{report}");

    // A title shared by several files is legal, but every [[link]] and
    // title-addressed API call to it is ambiguous, so name them explicitly.
    let duplicates = {
        let graph = Graph::open(vault)?;
        graph.duplicate_titles()?
    };
    if duplicates.is_empty() {
        println!("no ambiguous note titles");
    } else {
        println!("{} ambiguous note title(s):", duplicates.len());
        for (title, keys) in duplicates {
            println!("  {title} -> {}", keys.join(", "));
        }
        println!("  (each file is indexed separately; [[links]] to these titles are ambiguous)");
    }
    Ok(())
}

fn cmd_vault(action: VaultAction) -> Result<()> {
    let registry = Registry::open()?;
    match action {
        VaultAction::Add { name, path } => {
            let canonical = registry.add(&name, &path)?;
            let scope = Scope::load(&canonical)?;
            // Index it right away so cross-vault lookups work immediately.
            let report = indexer::reindex_in(&scope, false)?;
            println!("registered \"{name}\" at {}", display_path(&canonical));
            print_scope_summary(&scope)?;
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
            limit,
        } => cmd_search(&vault, &query, vault_name.as_deref(), all_vaults, limit)?,
        Command::Graph { all_vaults } => cmd_graph(&vault, all_vaults)?,
        Command::List => {
            for note in vault::list_notes(&vault)? {
                println!("{}", note.title);
            }
        }
        Command::Watch => watch::run(&vault)?,
        Command::Doctor => cmd_doctor(&vault)?,
        Command::Vault { action } => cmd_vault(action)?,
        Command::Update { check } => crate::update::run(check)?,
    }

    Ok(())
}
