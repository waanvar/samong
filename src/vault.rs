use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use regex::Regex;

use crate::scope::Scope;

/// Directory (relative to a vault root) where all generated indexes live.
/// Must always be reconstructible from the .md files alone.
pub const BRAIN_DIR: &str = ".brain";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// Display name: the filename without `.md`. Not unique within a vault.
    pub title: String,
    pub path: PathBuf,
    /// Stable identity: see [`relative_key`].
    pub key: String,
    /// Came from a `scope.include` root rather than the vault's own notes, so
    /// it is read-only: the directory belongs to a dependency, and an edit there
    /// would be erased by the next install. See [`Scope::is_reference`].
    pub reference: bool,
}

/// A note's identity inside a vault: its path relative to the vault root,
/// always slash-separated so a Windows and a Linux machine indexing the same
/// commit produce the same key.
///
/// Titles are *not* identities — one repo can hold twenty files named
/// `README.md` — so everything persisted (link graph, search index) keys off
/// this instead. It is also exactly how git names the same file, which is what
/// lets a central server ingest a vault straight from a commit.
pub fn relative_key(vault: &Path, path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.strip_prefix(vault).ok()?.components() {
        match component {
            Component::Normal(name) => parts.push(name.to_str()?),
            // `.` / `..` / a drive prefix cannot be part of an identity.
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    pub target: String,
    pub alias: Option<String>,
}

fn wikilink_pattern() -> Regex {
    Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").expect("static wikilink regex is valid")
}

/// Extract every `[[target]]` / `[[target|alias]]` occurrence from note content.
pub fn parse_wikilinks(content: &str) -> Vec<WikiLink> {
    wikilink_pattern()
        .captures_iter(content)
        .map(|caps| WikiLink {
            target: caps[1].trim().to_string(),
            alias: caps.get(2).map(|m| m.as_str().trim().to_string()),
        })
        .collect()
}

/// Full path for a note title inside the given vault.
pub fn note_path(vault: &Path, title: &str) -> PathBuf {
    vault.join(format!("{title}.md"))
}

/// Recover a note's title from its file path (file stem, i.e. filename without `.md`).
pub fn title_from_path(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// Every note in the vault, according to its scope rules (see [`crate::scope`]).
///
/// Loads the vault's config on each call. Callers that already hold a [`Scope`]
/// — the indexer, the watcher — should use [`list_notes_in`] instead.
pub fn list_notes(vault: &Path) -> Result<Vec<Note>> {
    list_notes_in(&Scope::load(vault)?)
}

/// Every note in scope, sorted by title then key so that duplicate titles keep
/// a stable, reproducible order.
pub fn list_notes_in(scope: &Scope) -> Result<Vec<Note>> {
    let root = scope.root();
    let mut notes = Vec::new();
    for path in scope.notes()? {
        let (Some(title), Some(key)) = (title_from_path(&path), relative_key(root, &path)) else {
            continue; // non-UTF-8 name: not addressable as a note
        };
        let reference = scope.is_reference(&key);
        notes.push(Note {
            title,
            path,
            key,
            reference,
        });
    }
    notes.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.key.cmp(&b.key)));
    Ok(notes)
}

/// Split a wikilink target of the form `vault-name/note-title`. Whether the
/// prefix actually names a registered vault is the caller's job to check —
/// unregistered prefixes stay plain in-vault targets (Obsidian folder links).
pub fn split_cross_vault(target: &str) -> Option<(&str, &str)> {
    let (vault_name, title) = target.split_once('/')?;
    if vault_name.is_empty() || title.is_empty() {
        return None;
    }
    Some((vault_name, title))
}

/// Every note in the vault carrying this title, in stable key order.
///
/// More than one is normal and legal — `README.md` and `docs/README.md` are
/// different notes that share a title.
pub fn find_notes(vault: &Path, title: &str) -> Result<Vec<Note>> {
    Ok(list_notes(vault)?
        .into_iter()
        .filter(|n| n.title == title)
        .collect())
}

/// Locate a note by title anywhere in the vault (including subdirectories).
/// When several share the title, the first in key order wins — deterministic,
/// though `samong doctor` will report the ambiguity.
pub fn find_note(vault: &Path, title: &str) -> Result<Option<Note>> {
    Ok(find_notes(vault, title)?.into_iter().next())
}

/// Rewrite every `[[old]]` / `[[old|alias]]` in `content` to point at `new`,
/// preserving aliases. Returns the new content and how many links were rewritten.
pub fn rewrite_wikilinks(content: &str, old: &str, new: &str) -> (String, usize) {
    let pattern = Regex::new(&format!(r"\[\[\s*{}\s*(\|[^\]]+)?\]\]", regex::escape(old)))
        .expect("escaped wikilink regex is valid");
    let mut count = 0;
    let rewritten = pattern
        .replace_all(content, |caps: &regex::Captures| {
            count += 1;
            let alias = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            format!("[[{new}{alias}]]")
        })
        .into_owned();
    (rewritten, count)
}

/// Create a new, empty note. Fails if a note with this title already exists.
pub fn create_note(vault: &Path, title: &str) -> Result<PathBuf> {
    let path = note_path(vault, title);
    if path.exists() {
        bail!("note \"{title}\" already exists at {}", path.display());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating parent dir for {}", path.display()))?;
    }
    fs::write(&path, format!("# {title}\n\n"))
        .with_context(|| format!("writing new note {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_wikilink() {
        let links = parse_wikilinks("see [[Rust]] for details");
        assert_eq!(
            links,
            vec![WikiLink {
                target: "Rust".to_string(),
                alias: None
            }]
        );
    }

    #[test]
    fn parses_aliased_wikilink() {
        let links = parse_wikilinks("see [[Rust Programming|Rust]] for details");
        assert_eq!(
            links,
            vec![WikiLink {
                target: "Rust Programming".to_string(),
                alias: Some("Rust".to_string())
            }]
        );
    }

    #[test]
    fn parses_multiple_wikilinks() {
        let links = parse_wikilinks("[[A]] links to [[B|Beta]] and [[C]]");
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].target, "A");
        assert_eq!(links[1].target, "B");
        assert_eq!(links[1].alias.as_deref(), Some("Beta"));
        assert_eq!(links[2].target, "C");
    }

    #[test]
    fn no_wikilinks_returns_empty() {
        assert!(parse_wikilinks("no links here").is_empty());
    }

    #[test]
    fn create_and_list_notes() {
        let dir = tempfile::tempdir().unwrap();
        create_note(dir.path(), "Hello World").unwrap();
        create_note(dir.path(), "Second Note").unwrap();

        let notes = list_notes(dir.path()).unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].title, "Hello World");
        assert_eq!(notes[1].title, "Second Note");
    }

    #[test]
    fn create_note_rejects_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        create_note(dir.path(), "Dup").unwrap();
        assert!(create_note(dir.path(), "Dup").is_err());
    }

    #[test]
    fn list_notes_skips_brain_dir() {
        let dir = tempfile::tempdir().unwrap();
        create_note(dir.path(), "Real Note").unwrap();
        let brain = dir.path().join(BRAIN_DIR);
        fs::create_dir_all(&brain).unwrap();
        fs::write(brain.join("fake.md"), "should not be listed").unwrap();

        let notes = list_notes(dir.path()).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "Real Note");
    }

    #[test]
    fn title_from_path_strips_extension() {
        let path = Path::new("/vault/My Note.md");
        assert_eq!(title_from_path(path).as_deref(), Some("My Note"));
    }

    #[test]
    fn rewrite_plain_and_aliased_wikilinks() {
        let content = "See [[Old Note]] and [[Old Note|nickname]] but not [[Other]]";
        let (rewritten, count) = rewrite_wikilinks(content, "Old Note", "New Note");
        assert_eq!(count, 2);
        assert_eq!(
            rewritten,
            "See [[New Note]] and [[New Note|nickname]] but not [[Other]]"
        );
    }

    #[test]
    fn rewrite_does_not_match_partial_titles() {
        let content = "[[Note]] and [[Note Two]]";
        let (rewritten, count) = rewrite_wikilinks(content, "Note", "Renamed");
        assert_eq!(count, 1);
        assert_eq!(rewritten, "[[Renamed]] and [[Note Two]]");
    }

    #[test]
    fn rewrite_escapes_regex_metacharacters_in_title() {
        let content = "link to [[C++ (lang)]]";
        let (rewritten, count) = rewrite_wikilinks(content, "C++ (lang)", "Cpp");
        assert_eq!(count, 1);
        assert_eq!(rewritten, "link to [[Cpp]]");
    }

    #[test]
    fn split_cross_vault_requires_both_halves() {
        assert_eq!(split_cross_vault("work/Note"), Some(("work", "Note")));
        assert_eq!(
            split_cross_vault("work/deep/Note"),
            Some(("work", "deep/Note"))
        );
        assert_eq!(split_cross_vault("plain"), None);
        assert_eq!(split_cross_vault("/Note"), None);
        assert_eq!(split_cross_vault("work/"), None);
    }

    #[test]
    fn relative_key_is_slash_separated_on_every_platform() {
        let vault = Path::new("/vault");
        assert_eq!(
            relative_key(vault, &Path::new("/vault").join("docs").join("API.md")).as_deref(),
            Some("docs/API.md")
        );
        assert_eq!(
            relative_key(vault, Path::new("/vault/Root.md")).as_deref(),
            Some("Root.md")
        );
        // The vault root itself is not a note, and nothing outside it has a key.
        assert_eq!(relative_key(vault, vault), None);
        assert_eq!(relative_key(vault, Path::new("/elsewhere/A.md")), None);
    }

    #[test]
    fn duplicate_titles_are_distinct_notes_with_distinct_keys() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("README.md"), "# root readme").unwrap();
        fs::write(dir.path().join("docs/README.md"), "# docs readme").unwrap();

        let notes = list_notes(dir.path()).unwrap();
        assert_eq!(notes.len(), 2, "both files are notes");
        let keys: Vec<&str> = notes.iter().map(|n| n.key.as_str()).collect();
        assert_eq!(keys, vec!["README.md", "docs/README.md"]);
        assert!(notes.iter().all(|n| n.title == "README"));

        // Both are findable by title; find_note picks the first key deterministically.
        assert_eq!(find_notes(dir.path(), "README").unwrap().len(), 2);
        assert_eq!(
            find_note(dir.path(), "README").unwrap().unwrap().key,
            "README.md"
        );
    }

    #[test]
    fn list_notes_applies_vault_scope() {
        let dir = tempfile::tempdir().unwrap();
        create_note(dir.path(), "Real").unwrap();
        let dep = dir.path().join("node_modules").join("left-pad");
        fs::create_dir_all(&dep).unwrap();
        fs::write(dep.join("README.md"), "# left-pad").unwrap();

        let notes = list_notes(dir.path()).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "Real");
    }

    #[test]
    fn find_note_locates_notes_in_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("area");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("Nested.md"), "# Nested\n").unwrap();

        let found = find_note(dir.path(), "Nested").unwrap().unwrap();
        assert_eq!(found.path, sub.join("Nested.md"));
        assert!(find_note(dir.path(), "Missing").unwrap().is_none());
    }
}
