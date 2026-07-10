use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use regex::Regex;

/// Directory (relative to a vault root) where all generated indexes live.
/// Must always be reconstructible from the .md files alone.
pub const BRAIN_DIR: &str = ".brain";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub title: String,
    pub path: PathBuf,
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

/// List every note (`*.md` file, skipping the `.brain` index directory) in the vault,
/// recursively.
pub fn list_notes(vault: &Path) -> Result<Vec<Note>> {
    let mut notes = Vec::new();
    for entry in walkdir::WalkDir::new(vault)
        .into_iter()
        .filter_entry(|e| e.file_name() != BRAIN_DIR)
    {
        let entry = entry.with_context(|| format!("walking vault {}", vault.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(title) = title_from_path(path) else {
            continue;
        };
        notes.push(Note {
            title,
            path: path.to_path_buf(),
        });
    }
    notes.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(notes)
}

/// Locate a note by title anywhere in the vault (including subdirectories).
pub fn find_note(vault: &Path, title: &str) -> Result<Option<Note>> {
    Ok(list_notes(vault)?.into_iter().find(|n| n.title == title))
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
