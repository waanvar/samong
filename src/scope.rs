//! Which files in a vault count as notes.
//!
//! A vault is very often the root of a source repository, so "every `.md` file
//! under this directory" is the wrong answer: it drags in `node_modules`,
//! `vendor/`, build output and every dependency's README. Scope is the rule
//! that separates a project's real notes from files that merely happen to be
//! Markdown.
//!
//! ## Determinism is the whole point
//!
//! The same vault must produce the same file set on every machine that indexes
//! it — two developers (or a central server ingesting the same repo) have to
//! agree on what a note is, or their indexes will disagree forever about which
//! files exist. So scope is decided *only* by files that live inside the vault
//! and travel with it in git:
//!
//! - `samong.toml`   — optional config, committed
//! - `.samongignore` — optional extra rules in gitignore syntax, committed
//! - `.gitignore`    — the repo's own rules, committed
//!
//! Per-machine sources are deliberately switched off: the global gitignore
//! (`~/.config/git/ignore`), `.git/info/exclude`, `.ignore`/`.rgignore`, and
//! `.gitignore` files in *parent* directories above the vault. Turning any of
//! them on would make the same commit index differently on two laptops.
//!
//! The default rule, when there is no config at all, is simply: **a note is a
//! `.md` file you would commit.** Nothing to configure for the common case.

use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use ignore::overrides::{Override, OverrideBuilder};
use ignore::WalkBuilder;
use serde::Deserialize;

use crate::vault::BRAIN_DIR;

/// Per-vault configuration. Lives at the vault root and is meant to be
/// committed, so every machine indexing this vault reads the same rules.
pub const CONFIG_FILE: &str = "samong.toml";

/// Extra ignore rules in gitignore syntax, including `!` negation to re-include
/// notes that `.gitignore` excludes. Also meant to be committed.
pub const IGNORE_FILE: &str = ".samongignore";

/// Directory names that are never notes, even in a vault that does not
/// gitignore them (or is not a git repo at all).
///
/// Kept deliberately short and boring: dependency trees only. Build output
/// (`target/`, `dist/`, `build/`) is *not* listed — it is virtually always
/// gitignored already, and those names are plausible enough as real note
/// folders that hard-coding them would silently swallow someone's notes.
const ALWAYS_EXCLUDE: &[&str] = &[
    "node_modules",
    "bower_components",
    "vendor",
    "site-packages",
    "__pycache__",
    "Pods",
];

/// Files scanned before [`Scope::audit`] gives up counting. An audit walks the
/// *unfiltered* tree on purpose, so it needs a stop.
const AUDIT_LIMIT: usize = 200_000;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub vault: VaultConfig,
    #[serde(default)]
    pub scope: ScopeConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultConfig {
    /// Name this vault answers to in `[[name/note]]` links. Lets a vault
    /// identify itself instead of relying on each machine's local registry.
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeConfig {
    /// Subdirectory holding the notes, relative to the vault root. Narrowing
    /// this is the cheapest way to scope a big repo (`notes_dir = "docs"`).
    #[serde(default = "default_notes_dir")]
    pub notes_dir: String,
    /// Extra exclusions in gitignore syntax, relative to `notes_dir`.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Directories to index *in addition* to the main scan, even when
    /// `.gitignore` or the dependency deny-list would exclude them. Paths are
    /// relative to the vault root.
    ///
    /// This is what makes vendored documentation reachable — the docs shipped
    /// inside `node_modules/next/dist/docs`, for instance. `.gitignore` answers
    /// "what do I distribute?"; a knowledge base has to answer "what do I learn
    /// from?", and those are not the same question.
    ///
    /// Notes found this way are *reference notes*: read-only, and machine-local
    /// by nature, because the directories they come from are not committed.
    /// See [`Scope::is_reference`].
    ///
    /// `exclude` applies to the main scan only. To leave part of an include root
    /// out, point the include at a narrower directory.
    #[serde(default)]
    pub include: Vec<String>,
    /// Whether `.gitignore` decides what is a note. On by default.
    #[serde(default = "default_true")]
    pub follow_gitignore: bool,
    /// Directory depth to descend, counting `notes_dir` as 0. `0` = unlimited.
    #[serde(default)]
    pub max_depth: usize,
}

impl Default for ScopeConfig {
    fn default() -> Self {
        Self {
            notes_dir: default_notes_dir(),
            exclude: Vec::new(),
            include: Vec::new(),
            follow_gitignore: true,
            max_depth: 0,
        }
    }
}

fn default_notes_dir() -> String {
    ".".to_string()
}

fn default_true() -> bool {
    true
}

impl Config {
    /// Read `samong.toml` from the vault root. A missing file means defaults;
    /// a malformed or unrecognized one is a hard error — a typo that silently
    /// widened the scope back to the whole repo is exactly the failure this
    /// module exists to prevent.
    pub fn load(vault: &Path) -> Result<Self> {
        let path = vault.join(CONFIG_FILE);
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
        };
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
    }
}

/// What a scan skipped, for reporting to a human. Produced by [`Scope::audit`].
pub struct ScopeAudit {
    /// `.md` files that are notes.
    pub included: usize,
    /// How many of `included` came from a `scope.include` root.
    pub reference: usize,
    /// `.md` files that exist but are not notes.
    pub skipped: usize,
    /// Of `skipped`, those inside a dependency directory. Split out because the
    /// two causes need different fixes — `scope.include` for this one,
    /// `.samongignore` or `follow_gitignore` for the other — and a reader who
    /// assumes "gitignored" reaches for the wrong lever.
    pub skipped_dependency: usize,
    /// Top-level directories the skipped files came from, largest first.
    pub skipped_by_dir: Vec<(String, usize)>,
    /// The audit hit [`AUDIT_LIMIT`] and stopped early; counts are lower bounds.
    pub truncated: bool,
}

/// One resolved `scope.include` entry.
#[derive(Debug, Clone)]
pub struct IncludeRoot {
    /// Exactly as written in the config, for error messages.
    pub declared: String,
    pub path: PathBuf,
    /// Slash-separated, vault-relative prefix that note keys under this root
    /// start with.
    pub prefix: String,
    /// Whether the directory is present on this machine right now.
    pub present: bool,
}

/// The compiled scope rules for one vault.
pub struct Scope {
    root: PathBuf,
    notes_root: PathBuf,
    config: Config,
    overrides: Override,
    include_roots: Vec<IncludeRoot>,
}

impl Scope {
    /// Compile the scope rules for `vault`, reading its committed config.
    pub fn load(vault: &Path) -> Result<Self> {
        Self::with_config(vault, Config::load(vault)?)
    }

    pub fn with_config(vault: &Path, config: Config) -> Result<Self> {
        let notes_root = resolve_notes_dir(vault, &config.scope.notes_dir)?;

        let mut builder = OverrideBuilder::new(&notes_root);
        for pattern in &config.scope.exclude {
            if pattern.starts_with('!') {
                bail!(
                    "scope.exclude pattern {pattern:?} must not start with \"!\" — \
                     every exclude pattern already excludes. Put negation rules in {IGNORE_FILE}"
                );
            }
            // An `!`-prefixed override glob is a blacklist entry; since every
            // pattern here gets one, no implicit "whitelist everything else"
            // behavior kicks in.
            builder
                .add(&format!("!{pattern}"))
                .with_context(|| format!("invalid scope.exclude pattern {pattern:?}"))?;
        }
        let overrides = builder
            .build()
            .context("compiling scope.exclude patterns")?;

        let mut include_roots = Vec::new();
        for declared in &config.scope.include {
            let relative = relative_inside_vault(declared, "scope.include")?;
            let path = vault.join(&relative);
            include_roots.push(IncludeRoot {
                declared: declared.clone(),
                prefix: relative.to_string_lossy().replace('\\', "/"),
                present: path.is_dir(),
                path,
            });
        }

        Ok(Self {
            root: vault.to_path_buf(),
            notes_root,
            config,
            overrides,
            include_roots,
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The vault root. Note identities are relative to this, never to
    /// `notes_root` — so narrowing `notes_dir` later does not rename every key.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory notes are scanned from — the vault root unless
    /// `scope.notes_dir` narrows it.
    pub fn notes_root(&self) -> &Path {
        &self.notes_root
    }

    pub fn include_roots(&self) -> &[IncludeRoot] {
        &self.include_roots
    }

    /// Declared include roots that are not on this machine.
    ///
    /// Never an error: `samong.toml` is committed but the directories it points
    /// at are usually not, so "missing" is the normal state before
    /// `npm install`, after a version bump, or on a server that only has the
    /// git history. It has to be *reported* though — silently losing a few
    /// hundred notes is how someone concludes that search is broken.
    pub fn missing_include_roots(&self) -> Vec<String> {
        self.include_roots
            .iter()
            .filter(|root| !root.present)
            .map(|root| root.declared.clone())
            .collect()
    }

    /// Is this note key a *reference note* — something pulled in by
    /// `scope.include` rather than part of the vault's own committed notes?
    ///
    /// Reference notes are read-only (they belong to a dependency, and any edit
    /// would be erased by the next install) and do not travel with the repo.
    pub fn is_reference(&self, key: &str) -> bool {
        self.include_roots.iter().any(|root| {
            key == root.prefix
                || (key.starts_with(&root.prefix)
                    && key.as_bytes().get(root.prefix.len()) == Some(&b'/'))
        })
    }

    /// Every note file in the vault, sorted, absolute paths.
    ///
    /// The main scan plus one extra scan per present include root. Keeping them
    /// as separate walks — instead of trying to punch a hole through the ignore
    /// rules — is deliberate: `filter_entry` prunes a directory before the
    /// walker ever looks inside it, and gitignore semantics cannot re-include a
    /// path whose parent is excluded. Fighting either one produces rules that
    /// look like they work and do not.
    pub fn notes(&self) -> Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        self.collect(self.walker().build(), &mut out)?;
        for root in self.include_roots.iter().filter(|root| root.present) {
            self.collect(self.include_walker(&root.path).build(), &mut out)?;
        }
        out.sort();
        // An include root inside the main scan would otherwise appear twice.
        out.dedup();
        Ok(out)
    }

    fn collect(&self, walk: ignore::Walk, out: &mut Vec<PathBuf>) -> Result<()> {
        for entry in walk {
            let entry = entry.with_context(|| format!("scanning {}", self.root.display()))?;
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            if !is_markdown(entry.path()) {
                continue;
            }
            out.push(entry.into_path());
        }
        Ok(())
    }

    /// Directories worth handing to a filesystem watcher: the notes root plus
    /// each of its included subdirectories. Watching these instead of the whole
    /// vault keeps a dependency tree from consuming the OS watch budget (on
    /// Linux a single `node_modules` can exhaust `max_user_watches` and break
    /// watch mode outright) and stops `npm install` from waking the indexer.
    pub fn watch_targets(&self) -> Result<Vec<PathBuf>> {
        let mut out = vec![self.notes_root.clone()];
        for entry in self.walker().max_depth(Some(1)).build() {
            let entry = entry.with_context(|| format!("scanning {}", self.root.display()))?;
            if entry.depth() == 0 || !entry.file_type().is_some_and(|t| t.is_dir()) {
                continue;
            }
            out.push(entry.into_path());
        }
        // Include roots sit outside the pruned tree, so they need their own watch.
        out.extend(
            self.include_roots
                .iter()
                .filter(|root| root.present)
                .map(|root| root.path.clone()),
        );
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// Could `path` be a note in this vault?
    ///
    /// Cheap, allocation-light, and **structural only**: it checks the
    /// extension, the notes root, excluded directory names, depth, and
    /// `scope.exclude`, but does not consult `.gitignore` (that needs the full
    /// walk). A filesystem watcher uses this to decide whether an event is
    /// worth a rescan, where a false positive costs one cheap incremental scan
    /// and nothing else — [`Scope::notes`] stays the authority on what a note is.
    pub fn may_include(&self, path: &Path) -> bool {
        if !is_markdown(path) {
            return false;
        }
        // Reference notes live under directories the main rules prune, so they
        // have to be checked first or every rule below would reject them.
        if self
            .include_roots
            .iter()
            .any(|root| root.present && path.starts_with(&root.path))
        {
            return true;
        }
        let Ok(rel) = path.strip_prefix(&self.notes_root) else {
            return false;
        };
        let mut depth = 0;
        for component in rel.components() {
            let Component::Normal(name) = component else {
                return false; // `..` or a prefix: not addressable inside the vault
            };
            depth += 1;
            let name = name.to_string_lossy();
            if name.starts_with('.') || ALWAYS_EXCLUDE.contains(&name.as_ref()) {
                return false;
            }
        }
        if self.config.scope.max_depth > 0 && depth > self.config.scope.max_depth {
            return false;
        }
        !self.overrides.matched(rel, false).is_ignore()
    }

    /// Compare the scoped scan against an unfiltered one, so a human can see
    /// what was left out and why.
    ///
    /// This deliberately descends the excluded directories, which is exactly
    /// the expensive walk the rest of Samong avoids — call it on explicit user
    /// action (`vault add`, `doctor`), never on the reindex path.
    pub fn audit(&self) -> Result<ScopeAudit> {
        use std::collections::HashMap;

        let included: Vec<PathBuf> = self.notes()?;
        let included_set: std::collections::HashSet<&Path> =
            included.iter().map(|p| p.as_path()).collect();

        let reference = included
            .iter()
            .filter(|path| {
                crate::vault::relative_key(&self.root, path)
                    .is_some_and(|key| self.is_reference(&key))
            })
            .count();

        let mut skipped = 0;
        let mut skipped_dependency = 0;
        let mut per_dir: HashMap<String, usize> = HashMap::new();
        let mut scanned = 0;
        let mut truncated = false;

        // Only the two rules that are never negotiable: our own index dir, and
        // dot-directories (`.git` above all — walking it is pure waste).
        let walker = WalkBuilder::new(&self.root)
            .hidden(true)
            .ignore(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .parents(false)
            .follow_links(false)
            .filter_entry(|entry| !is_brain_dir(entry))
            .build();

        for entry in walker {
            scanned += 1;
            if scanned > AUDIT_LIMIT {
                truncated = true;
                break;
            }
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_some_and(|t| t.is_file()) || !is_markdown(entry.path()) {
                continue;
            }
            if included_set.contains(entry.path()) {
                continue;
            }
            skipped += 1;
            if in_dependency_dir(&self.root, entry.path()) {
                skipped_dependency += 1;
            }
            *per_dir.entry(self.top_level_of(entry.path())).or_default() += 1;
        }

        let mut skipped_by_dir: Vec<(String, usize)> = per_dir.into_iter().collect();
        skipped_by_dir.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        Ok(ScopeAudit {
            included: included.len(),
            reference,
            skipped,
            skipped_dependency,
            skipped_by_dir,
            truncated,
        })
    }

    /// Top-level directory of a path inside the vault, for grouping in reports.
    fn top_level_of(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .ok()
            .and_then(|rel| rel.components().next())
            .and_then(|c| match c {
                Component::Normal(name) => Some(name.to_string_lossy().to_string()),
                _ => None,
            })
            .unwrap_or_else(|| ".".to_string())
    }

    /// Walker for one include root: the user has explicitly asked for this
    /// directory, so neither `.gitignore` nor the dependency deny-list applies
    /// inside it. Hidden directories and our own index stay excluded.
    fn include_walker(&self, root: &Path) -> WalkBuilder {
        let mut builder = WalkBuilder::new(root);
        builder
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .parents(false)
            .ignore(false)
            .hidden(true)
            .follow_links(false)
            .filter_entry(|entry| !is_brain_dir(entry));
        builder
    }

    /// The one place walker settings are defined, so every scan — notes, watch
    /// targets, audit baseline — agrees on the rules.
    fn walker(&self) -> WalkBuilder {
        let mut builder = WalkBuilder::new(&self.notes_root);
        builder
            // Committed, inside the vault: these decide what a note is.
            .git_ignore(self.config.scope.follow_gitignore)
            .add_custom_ignore_filename(IGNORE_FILE)
            // Apply .gitignore even when the vault is not a git repo, so a
            // notes-only vault behaves the same as a checked-out one.
            .require_git(false)
            // Per-machine or outside the vault: would break determinism.
            .git_global(false)
            .git_exclude(false)
            .parents(false)
            .ignore(false)
            // `.brain`, `.git`, `.obsidian` and friends are never notes.
            .hidden(true)
            // Symlinks can leave the vault entirely, or loop.
            .follow_links(false)
            .overrides(self.overrides.clone())
            .filter_entry(|entry| !is_always_excluded(entry));
        if self.config.scope.max_depth > 0 {
            builder.max_depth(Some(self.config.scope.max_depth));
        }
        builder
    }
}

fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("md")
}

/// Does any directory between the vault root and this path carry a name from
/// [`ALWAYS_EXCLUDE`]? Used only to explain *why* a file was skipped.
fn in_dependency_dir(vault: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(vault) else {
        return false;
    };
    rel.components().any(|component| match component {
        Component::Normal(name) => name
            .to_str()
            .is_some_and(|name| ALWAYS_EXCLUDE.contains(&name)),
        _ => false,
    })
}

fn is_brain_dir(entry: &ignore::DirEntry) -> bool {
    entry.depth() > 0
        && entry.file_type().is_some_and(|t| t.is_dir())
        && entry.file_name() == BRAIN_DIR
}

/// Never applied at depth 0: a vault may legitimately *be* a directory called
/// `vendor`, and the user pointed us at it on purpose.
fn is_always_excluded(entry: &ignore::DirEntry) -> bool {
    if entry.depth() == 0 || !entry.file_type().is_some_and(|t| t.is_dir()) {
        return false;
    }
    let Some(name) = entry.file_name().to_str() else {
        return false;
    };
    name == BRAIN_DIR || ALWAYS_EXCLUDE.contains(&name)
}

/// Validate a configured path as relative and contained: a vault must never be
/// talked into indexing something outside itself.
fn relative_inside_vault(raw: &str, field: &str) -> Result<PathBuf> {
    let trimmed = raw.trim().trim_start_matches("./").replace('\\', "/");
    let candidate = Path::new(&trimmed);
    if trimmed.is_empty()
        || candidate.is_absolute()
        || candidate
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        bail!(
            "{field} {raw:?} must be a relative path inside the vault \
             (no \"..\", no absolute paths)"
        );
    }
    Ok(candidate.to_path_buf())
}

/// Resolve `scope.notes_dir` against the vault root.
fn resolve_notes_dir(vault: &Path, notes_dir: &str) -> Result<PathBuf> {
    let trimmed = notes_dir.trim().trim_start_matches("./");
    if trimmed.is_empty() || trimmed == "." {
        return Ok(vault.to_path_buf());
    }
    Ok(vault.join(relative_inside_vault(notes_dir, "scope.notes_dir")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a file, creating parents. Returns the absolute path.
    fn write(root: &Path, rel: &str, body: &str) -> PathBuf {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, body).unwrap();
        path
    }

    /// Note paths relative to the vault, slash-separated, for stable asserts.
    fn scoped(root: &Path) -> Vec<String> {
        Scope::load(root)
            .unwrap()
            .notes()
            .unwrap()
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn dependency_trees_are_never_notes() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "PROJECT_OVERVIEW.md", "# overview");
        write(dir.path(), "node_modules/left-pad/README.md", "# left-pad");
        write(dir.path(), "node_modules/a/b/c/CHANGELOG.md", "# changelog");
        write(dir.path(), "vendor/gem/README.md", "# gem");
        write(dir.path(), "api/__pycache__/notes.md", "# pyc");

        assert_eq!(scoped(dir.path()), vec!["PROJECT_OVERVIEW.md"]);
    }

    #[test]
    fn gitignored_files_are_not_notes_even_without_a_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), ".gitignore", "build/\nscratch.md\n");
        write(dir.path(), "AGENTS.md", "# agents");
        write(dir.path(), "scratch.md", "# scratch");
        write(dir.path(), "build/generated.md", "# generated");

        assert_eq!(scoped(dir.path()), vec!["AGENTS.md"]);
    }

    #[test]
    fn samongignore_adds_rules_and_can_negate_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        // The repo ignores its local notes; the vault wants them back.
        write(dir.path(), ".gitignore", "notes/\n");
        write(dir.path(), IGNORE_FILE, "!notes/\ndrafts/\n");
        write(dir.path(), "notes/Kept.md", "# kept");
        write(dir.path(), "drafts/Dropped.md", "# dropped");
        write(dir.path(), "Root.md", "# root");

        assert_eq!(scoped(dir.path()), vec!["Root.md", "notes/Kept.md"]);
    }

    #[test]
    fn hidden_dirs_and_brain_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Real.md", "# real");
        write(dir.path(), ".git/description.md", "# git internals");
        write(dir.path(), ".obsidian/plugin/readme.md", "# plugin");
        write(dir.path(), &format!("{BRAIN_DIR}/stale.md"), "# index");

        assert_eq!(scoped(dir.path()), vec!["Real.md"]);
    }

    #[test]
    fn per_machine_ignore_sources_are_ignored() {
        // `.git/info/exclude` and `.ignore` are not committed with the repo (or
        // not part of it at all), so they must not change what a note is.
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), ".git/info/exclude", "Excluded.md\n");
        write(dir.path(), ".ignore", "Excluded.md\n");
        write(dir.path(), "Excluded.md", "# still a note");

        assert_eq!(scoped(dir.path()), vec!["Excluded.md"]);
    }

    #[test]
    fn parent_gitignore_above_the_vault_is_ignored() {
        let outer = tempfile::tempdir().unwrap();
        write(outer.path(), ".gitignore", "Note.md\n");
        let vault = outer.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        write(&vault, "Note.md", "# note");

        assert_eq!(scoped(&vault), vec!["Note.md"]);
    }

    #[test]
    fn notes_dir_narrows_the_scan() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), CONFIG_FILE, "[scope]\nnotes_dir = \"docs\"\n");
        write(dir.path(), "docs/Guide.md", "# guide");
        write(dir.path(), "src/lib.md", "# not a note here");

        assert_eq!(scoped(dir.path()), vec!["docs/Guide.md"]);
    }

    #[test]
    fn exclude_patterns_apply() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\nexclude = [\"archive/**\", \"TODO.md\"]\n",
        );
        write(dir.path(), "Keep.md", "# keep");
        write(dir.path(), "TODO.md", "# todo");
        write(dir.path(), "archive/2019/Old.md", "# old");

        assert_eq!(scoped(dir.path()), vec!["Keep.md"]);
    }

    #[test]
    fn follow_gitignore_can_be_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\nfollow_gitignore = false\n",
        );
        write(dir.path(), ".gitignore", "Generated.md\n");
        write(dir.path(), "Generated.md", "# generated");

        assert_eq!(scoped(dir.path()), vec!["Generated.md"]);
        // The hard-coded dependency list still applies.
        write(dir.path(), "node_modules/x/README.md", "# dep");
        assert_eq!(scoped(dir.path()), vec!["Generated.md"]);
    }

    #[test]
    fn max_depth_limits_descent() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), CONFIG_FILE, "[scope]\nmax_depth = 1\n");
        write(dir.path(), "Top.md", "# top");
        write(dir.path(), "deep/Nested.md", "# nested");

        assert_eq!(scoped(dir.path()), vec!["Top.md"]);
    }

    #[test]
    fn malformed_or_unknown_config_fails_loudly() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), CONFIG_FILE, "[scope]\nnotes_dir = \n");
        assert!(Scope::load(dir.path()).is_err(), "malformed toml must fail");

        // A typo must not silently fall back to indexing everything.
        write(dir.path(), CONFIG_FILE, "[scope]\nexcludes = [\"a\"]\n");
        assert!(Scope::load(dir.path()).is_err(), "unknown key must fail");
    }

    #[test]
    fn notes_dir_cannot_escape_the_vault() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\nnotes_dir = \"../evil\"\n",
        );
        assert!(Scope::load(dir.path()).is_err());
    }

    #[test]
    fn exclude_rejects_negation_with_a_pointer_to_samongignore() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), CONFIG_FILE, "[scope]\nexclude = [\"!keep\"]\n");
        let err = Scope::load(dir.path())
            .err()
            .expect("negated exclude pattern must be rejected")
            .to_string();
        assert!(
            err.contains(IGNORE_FILE),
            "error should point at {IGNORE_FILE}"
        );
    }

    #[test]
    fn vault_name_comes_from_config_when_present() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), CONFIG_FILE, "[vault]\nname = \"myproject\"\n");
        let scope = Scope::load(dir.path()).unwrap();
        assert_eq!(scope.config().vault.name.as_deref(), Some("myproject"));
    }

    #[test]
    fn no_config_means_sensible_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let scope = Scope::load(dir.path()).unwrap();
        assert!(scope.config().vault.name.is_none());
        assert!(scope.config().scope.follow_gitignore);
        assert_eq!(scope.notes_root(), dir.path());
    }

    #[test]
    fn may_include_matches_the_scan_for_structural_rules() {
        let dir = tempfile::tempdir().unwrap();
        let scope = Scope::load(dir.path()).unwrap();

        assert!(scope.may_include(&dir.path().join("Note.md")));
        assert!(scope.may_include(&dir.path().join("area/Deep.md")));
        assert!(!scope.may_include(&dir.path().join("Note.txt")));
        assert!(!scope.may_include(&dir.path().join("node_modules/x/README.md")));
        assert!(!scope.may_include(&dir.path().join(format!("{BRAIN_DIR}/graph.md"))));
        assert!(!scope.may_include(&dir.path().join(".git/x.md")));
        assert!(!scope.may_include(Path::new("/somewhere/else/Note.md")));
    }

    #[test]
    fn may_include_honors_exclude_and_depth() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\nmax_depth = 1\nexclude = [\"archive/**\"]\n",
        );
        let scope = Scope::load(dir.path()).unwrap();

        assert!(scope.may_include(&dir.path().join("Top.md")));
        assert!(!scope.may_include(&dir.path().join("deep/Nested.md")));
        assert!(!scope.may_include(&dir.path().join("archive/Old.md")));
    }

    #[test]
    fn watch_targets_skip_dependency_trees() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "docs/Guide.md", "# guide");
        write(dir.path(), "node_modules/x/README.md", "# dep");
        write(dir.path(), ".git/config", "");

        let targets: Vec<String> = Scope::load(dir.path())
            .unwrap()
            .watch_targets()
            .unwrap()
            .iter()
            .map(|p| {
                p.strip_prefix(dir.path())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();

        assert_eq!(targets, vec!["", "docs"]);
    }

    /// The lever that makes vendored documentation reachable: `.gitignore` says
    /// what to distribute, `scope.include` says what to learn from.
    #[test]
    fn include_reaches_into_a_gitignored_dependency_directory() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), ".gitignore", "node_modules/\n");
        write(dir.path(), "PROJECT_OVERVIEW.md", "# overview");
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\ninclude = [\"node_modules/next/dist/docs\"]\n",
        );
        write(
            dir.path(),
            "node_modules/next/dist/docs/01-app/installation.md",
            "# installation",
        );
        write(
            dir.path(),
            "node_modules/next/dist/docs/routing.md",
            "# routing",
        );
        // Not under the include root: still excluded.
        write(dir.path(), "node_modules/next/README.md", "# next readme");
        write(dir.path(), "node_modules/left-pad/README.md", "# left-pad");

        assert_eq!(
            scoped(dir.path()),
            vec![
                "PROJECT_OVERVIEW.md",
                "node_modules/next/dist/docs/01-app/installation.md",
                "node_modules/next/dist/docs/routing.md",
            ]
        );
    }

    #[test]
    fn included_notes_are_marked_as_reference_notes() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\ninclude = [\"vendor/docs\"]\n",
        );
        let scope = Scope::load(dir.path()).unwrap();

        assert!(scope.is_reference("vendor/docs/guide.md"));
        assert!(scope.is_reference("vendor/docs/deep/guide.md"));
        // Sibling paths that merely share a prefix are not inside the root.
        assert!(!scope.is_reference("vendor/docs-extra/guide.md"));
        assert!(!scope.is_reference("vendor/other/guide.md"));
        assert!(!scope.is_reference("Own Note.md"));
    }

    /// `samong.toml` is committed; the directories it points at usually are not.
    /// A missing root is the normal state, not a failure.
    #[test]
    fn a_missing_include_root_is_tolerated_and_reported() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Own.md", "# own");
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\ninclude = [\"node_modules/next/dist/docs\", \"vendor/docs\"]\n",
        );

        // Scanning still works and still finds the vault's own notes.
        assert_eq!(scoped(dir.path()), vec!["Own.md"]);

        let scope = Scope::load(dir.path()).unwrap();
        assert_eq!(
            scope.missing_include_roots(),
            vec![
                "node_modules/next/dist/docs".to_string(),
                "vendor/docs".to_string()
            ]
        );
        assert!(scope.include_roots().iter().all(|root| !root.present));
    }

    #[test]
    fn include_root_present_and_missing_are_distinguished() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "vendor/here/doc.md", "# doc");
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\ninclude = [\"vendor/here\", \"vendor/gone\"]\n",
        );

        let scope = Scope::load(dir.path()).unwrap();
        assert_eq!(
            scope.missing_include_roots(),
            vec!["vendor/gone".to_string()]
        );
        assert_eq!(scoped(dir.path()), vec!["vendor/here/doc.md"]);
    }

    #[test]
    fn an_include_root_inside_the_normal_scan_is_not_listed_twice() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "docs/Guide.md", "# guide");
        write(dir.path(), CONFIG_FILE, "[scope]\ninclude = [\"docs\"]\n");

        assert_eq!(scoped(dir.path()), vec!["docs/Guide.md"]);
    }

    #[test]
    fn include_cannot_escape_the_vault() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\ninclude = [\"../elsewhere\"]\n",
        );
        assert!(Scope::load(dir.path()).is_err());

        write(dir.path(), CONFIG_FILE, "[scope]\ninclude = [\"/etc\"]\n");
        assert!(Scope::load(dir.path()).is_err());
    }

    #[test]
    fn watch_covers_include_roots_and_may_include_accepts_them() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), ".gitignore", "node_modules/\n");
        write(dir.path(), "node_modules/next/dist/docs/a.md", "# a");
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\ninclude = [\"node_modules/next/dist/docs\"]\n",
        );
        let scope = Scope::load(dir.path()).unwrap();

        // The watcher must accept edits inside the include root...
        assert!(scope.may_include(&dir.path().join("node_modules/next/dist/docs/a.md")));
        // ...without reopening the rest of the dependency tree.
        assert!(!scope.may_include(&dir.path().join("node_modules/left-pad/README.md")));

        let targets: Vec<String> = scope
            .watch_targets()
            .unwrap()
            .iter()
            .map(|p| {
                p.strip_prefix(dir.path())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert!(
            targets.contains(&"node_modules/next/dist/docs".to_string()),
            "include roots need their own watch: {targets:?}"
        );
    }

    /// A vault may be rooted *inside* a dependency directory, because the deny
    /// list never applies at depth 0 — the user pointed us here on purpose.
    ///
    /// This pins that behaviour deliberately: it reads like an accident of
    /// `is_always_excluded`, so a future "tidy-up" that checked the whole path
    /// instead of the entry name would silently empty such a vault.
    #[test]
    fn a_vault_rooted_inside_a_dependency_directory_still_works() {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("node_modules/next/dist/docs");
        fs::create_dir_all(&docs).unwrap();
        fs::write(docs.join("installation.md"), "# installation").unwrap();
        fs::write(docs.join("routing.md"), "# routing").unwrap();

        assert_eq!(scoped(&docs), vec!["installation.md", "routing.md"]);
    }

    #[test]
    fn audit_separates_reference_notes_and_dependency_skips() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), ".gitignore", "node_modules/\nsecret.md\n");
        write(dir.path(), "Own.md", "# own");
        write(dir.path(), "secret.md", "# gitignored, not a dependency");
        write(
            dir.path(),
            CONFIG_FILE,
            "[scope]\ninclude = [\"node_modules/next/dist/docs\"]\n",
        );
        write(dir.path(), "node_modules/next/dist/docs/a.md", "# a");
        write(dir.path(), "node_modules/next/dist/docs/b.md", "# b");
        write(dir.path(), "node_modules/left-pad/README.md", "# dep");

        let audit = Scope::load(dir.path()).unwrap().audit().unwrap();
        assert_eq!(audit.included, 3, "own note + two reference notes");
        assert_eq!(audit.reference, 2);
        assert_eq!(audit.skipped, 2, "left-pad readme + the gitignored note");
        assert_eq!(
            audit.skipped_dependency, 1,
            "only the left-pad readme is inside a dependency dir"
        );
    }

    #[test]
    fn audit_reports_what_was_left_out() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "PROJECT_OVERVIEW.md", "# overview");
        write(dir.path(), "CLAUDE.md", "# claude");
        for i in 0..5 {
            write(
                dir.path(),
                &format!("node_modules/dep{i}/README.md"),
                "# dep",
            );
        }
        write(dir.path(), "vendor/v/CHANGELOG.md", "# changelog");

        let audit = Scope::load(dir.path()).unwrap().audit().unwrap();
        assert_eq!(audit.included, 2);
        assert_eq!(audit.skipped, 6);
        assert_eq!(
            audit.skipped_by_dir,
            vec![("node_modules".to_string(), 5), ("vendor".to_string(), 1)]
        );
        assert!(!audit.truncated);
    }
}
