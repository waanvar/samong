//! Installing someone else's vault into your own.
//!
//! A vault you were given — bought, shared, published — arrives as a git
//! repository and lands as **reference notes**: same graph, same search, same
//! `[[link]]` space as the notes you wrote, but read-only. That reuses the
//! machinery built for vendored documentation in Phase 13 rather than inventing a
//! second kind of vault, and it is the same judgement: one project, one brain.
//!
//! Read-only is not decoration. An edit would be erased by the next `update`, and
//! the content is not the reader's to change.
//!
//! # Why the `git` binary rather than a Rust git library
//!
//! Because of authentication. A vault someone sells lives in a private repository,
//! and reaching it means SSH agents, credential helpers, hardware keys, SSO device
//! flows and 2FA tokens — everything the user has *already* configured for `git`.
//! Embedding a library means reimplementing that badly, and the first person to
//! hit a config we did not anticipate simply cannot get what they paid for. The
//! cost is a dependency on a program almost every user of this already has, and a
//! clear message when they do not.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::scope::{Scope, CONFIG_FILE};

/// Marker written into `.gitignore` so the block can be recognised later.
const GITIGNORE_MARKER: &str = "# installed vaults (samong vault install)";

fn git(args: &[&str], cwd: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| {
            anyhow::anyhow!(
                "could not run `git`: {error}\n\
                 Installing a vault clones a git repository, so git has to be on PATH. \
                 Install it from https://git-scm.com and try again."
            )
        })?;
    if !output.status.success() {
        bail!(
            "git {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Directory name implied by a git URL: the last segment, minus `.git`.
pub fn name_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim_end_matches('/').trim_end_matches(".git");
    let last = trimmed
        .rsplit(['/', ':'])
        .find(|part| !part.is_empty())?
        .to_string();
    // A name that is not a plain directory component would escape the vault or
    // confuse the scope walker.
    if last.is_empty() || last.contains(['/', '\\']) || last.starts_with('.') {
        return None;
    }
    Some(last)
}

pub struct Installed {
    pub name: String,
    /// Path relative to the vault root, slash-separated — what `scope.include`
    /// and `.gitignore` both want.
    pub relative: String,
    pub absolute: PathBuf,
}

/// Clone a vault into this one and wire it up.
pub fn install(vault: &Path, url: &str, into: &str, name: Option<&str>) -> Result<Installed> {
    let name = match name {
        Some(explicit) => explicit.to_string(),
        None => name_from_url(url)
            .context("cannot tell what to call this vault from its URL — pass --name")?,
    };
    let relative = format!("{}/{name}", into.trim_matches('/').replace('\\', "/"));
    let absolute = vault.join(&relative);

    if absolute.exists() && std::fs::read_dir(&absolute)?.next().is_some() {
        bail!(
            "{relative} already exists and is not empty — \
             use `samong vault update {name}` to refresh it"
        );
    }
    if let Some(parent) = absolute.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    git(&["clone", "--quiet", url, &relative], vault)?;

    Ok(Installed {
        name,
        relative,
        absolute,
    })
}

/// Add the installed path to `scope.include` in the vault's own manifest.
///
/// Edited with a format-preserving parser: `samong.toml` is a file people write
/// by hand and comment, and a round-trip through a plain deserialiser would hand
/// it back stripped of both.
pub fn add_to_scope_include(vault: &Path, relative: &str) -> Result<bool> {
    let path = vault.join(CONFIG_FILE);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = existing
        .parse()
        .with_context(|| format!("parsing {}", path.display()))?;

    let scope = doc
        .entry("scope")
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    let Some(table) = scope.as_table_mut() else {
        bail!("[scope] in {CONFIG_FILE} is not a table");
    };
    let include = table
        .entry("include")
        .or_insert(toml_edit::value(toml_edit::Array::new()));
    let Some(array) = include.as_array_mut() else {
        bail!("scope.include in {CONFIG_FILE} is not an array");
    };
    if array
        .iter()
        .any(|v| v.as_str().is_some_and(|s| s == relative))
    {
        return Ok(false);
    }
    array.push(relative);
    std::fs::write(&path, doc.to_string())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

/// Keep an installed vault out of the reader's own git history.
///
/// Without this the buyer commits somebody else's notes into their repository,
/// which for a vault that was sold to them is a licence breach they never chose
/// to make — and the default should not be the one that gets people in trouble.
pub fn add_to_gitignore(vault: &Path, relative: &str) -> Result<bool> {
    let path = vault.join(".gitignore");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let entry = format!("/{relative}/");
    if existing.lines().any(|line| line.trim() == entry) {
        return Ok(false);
    }
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.contains(GITIGNORE_MARKER) {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(GITIGNORE_MARKER);
        out.push('\n');
        out.push_str("# Someone else's notes. Committing them here would redistribute\n");
        out.push_str("# content that is not yours to redistribute.\n");
    }
    out.push_str(&entry);
    out.push('\n');
    std::fs::write(&path, out).with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

/// An installed vault as found on disk: a `scope.include` root that is a git
/// checkout. Nothing extra is recorded anywhere — the checkout is its own
/// provenance, and a registry entry could only drift from it.
pub struct Installation {
    pub name: String,
    pub path: PathBuf,
}

pub fn installed(scope: &Scope) -> Vec<Installation> {
    scope
        .include_roots()
        .iter()
        .filter(|root| root.present && root.path.join(".git").exists())
        .map(|root| Installation {
            name: root
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| root.declared.clone()),
            path: root.path.clone(),
        })
        .collect()
}

pub struct UpdateResult {
    pub name: String,
    pub before: String,
    pub after: String,
}

impl UpdateResult {
    pub fn changed(&self) -> bool {
        self.before != self.after
    }
}

/// `git pull` one installation, reporting the commit it moved between.
pub fn update(installation: &Installation) -> Result<UpdateResult> {
    let before = git(&["rev-parse", "--short", "HEAD"], &installation.path)?;
    git(&["pull", "--quiet", "--ff-only"], &installation.path)?;
    let after = git(&["rev-parse", "--short", "HEAD"], &installation.path)?;
    Ok(UpdateResult {
        name: installation.name.clone(),
        before,
        after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_name_is_taken_from_the_url() {
        assert_eq!(
            name_from_url("https://github.com/me/sre-handbook.git").as_deref(),
            Some("sre-handbook")
        );
        assert_eq!(
            name_from_url("git@github.com:me/sre-handbook.git").as_deref(),
            Some("sre-handbook")
        );
        assert_eq!(
            name_from_url("https://example.com/team/notes/").as_deref(),
            Some("notes")
        );
    }

    /// A URL must never be able to choose where the clone lands. Anything that
    /// is not a plain directory component is refused rather than sanitised —
    /// sanitising invites arguments about whether the sanitiser is complete.
    #[test]
    fn a_url_cannot_name_a_directory_that_escapes_the_vault() {
        assert_eq!(name_from_url("https://example.com/x/..").as_deref(), None);
        assert_eq!(name_from_url("https://example.com/x/.hidden"), None);
        // A URL with no path at all gives the host, which is a bad name but not a
        // dangerous one; `--name` exists for that.
        assert_eq!(
            name_from_url("https://example.com/").as_deref(),
            Some("example.com")
        );
    }

    #[test]
    fn scope_include_gains_the_path_once_and_keeps_the_file_readable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CONFIG_FILE),
            "# my notes\n[vault]\nname = \"mine\"  # keep this comment\n",
        )
        .unwrap();

        assert!(add_to_scope_include(dir.path(), "vendor/handbook").unwrap());
        let text = std::fs::read_to_string(dir.path().join(CONFIG_FILE)).unwrap();
        assert!(text.contains("vendor/handbook"));
        assert!(
            text.contains("# keep this comment"),
            "a hand-written config must survive being edited by us:\n{text}"
        );
        assert!(text.contains("# my notes"));

        // Installing the same thing twice must not duplicate the entry.
        assert!(!add_to_scope_include(dir.path(), "vendor/handbook").unwrap());
        let again = std::fs::read_to_string(dir.path().join(CONFIG_FILE)).unwrap();
        assert_eq!(again.matches("vendor/handbook").count(), 1);
    }

    #[test]
    fn scope_include_works_when_there_is_no_config_yet() {
        let dir = tempfile::tempdir().unwrap();
        assert!(add_to_scope_include(dir.path(), "vendor/handbook").unwrap());
        let text = std::fs::read_to_string(dir.path().join(CONFIG_FILE)).unwrap();
        let parsed: toml::Table = toml::from_str(&text).expect("what we write must be valid TOML");
        assert_eq!(
            parsed["scope"]["include"][0].as_str(),
            Some("vendor/handbook")
        );
    }

    #[test]
    fn gitignore_gains_the_path_with_a_reason_attached() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();

        assert!(add_to_gitignore(dir.path(), "vendor/handbook").unwrap());
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(text.starts_with("target/\n"), "existing rules survive");
        assert!(text.contains("/vendor/handbook/"));
        assert!(
            text.contains("not yours to redistribute"),
            "the reason has to travel with the rule, or someone deletes it"
        );

        assert!(!add_to_gitignore(dir.path(), "vendor/handbook").unwrap());
        let again = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(again.matches("/vendor/handbook/").count(), 1);
    }

    #[test]
    fn gitignore_is_created_when_the_vault_has_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(add_to_gitignore(dir.path(), "vendor/x").unwrap());
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(text.contains("/vendor/x/"));
    }
}
