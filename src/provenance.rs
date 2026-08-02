//! Where a note came from, when it did not come from you.
//!
//! Reference notes pulled in by `scope.include` sit in the same graph and the
//! same search results as notes the reader wrote. That is the point — one
//! project, one brain — but it means a result can be somebody else's work, and
//! the reader cannot tell from a path they chose themselves. Whoever installed
//! a vault into `vendor/h/` is the only person who knows what `vendor/h/` is,
//! and they will not be the only person reading these results.
//!
//! What travels with a hit is what the source vault says about itself: its name,
//! and its licence. The licence is the part that matters, because the moment
//! search *becomes* dangerous is the moment someone copies a paragraph out of a
//! result into their own notes — and by then the fact that it was quoted from a
//! bought vault is gone. Attribution has to arrive with the content, not be
//! looked up afterwards by someone who already knows to ask.

use serde::Serialize;

use crate::scope::{Config, Scope};

/// One installed vault, as it describes itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Source {
    /// The `scope.include` path this came from, exactly as declared. The stable
    /// identifier: names and licences are the source's to change, this is the
    /// reader's own address for it.
    pub root: String,
    /// `[vault] name` from the source's manifest, falling back to the directory
    /// name — never blank, because a hit with no attribution at all reads as if
    /// there were nothing to attribute.
    pub name: String,
    pub license: Option<String>,
    pub version: Option<String>,
}

impl Source {
    /// One line, for a place that has room for one line.
    pub fn label(&self) -> String {
        match &self.license {
            Some(license) => format!("{} · {license}", self.name),
            // Said plainly rather than left off. "No licence stated" is not a
            // missing detail, it is the answer: the reader may not have been
            // given permission to reuse any of this.
            None => format!("{} · licence not stated", self.name),
        }
    }
}

/// The installed vaults of one vault, ready to be asked about any note key.
///
/// Built once per search rather than once per hit: it reads a file per include
/// root, and a query returning twenty hits from one source would otherwise read
/// the same manifest twenty times.
#[derive(Debug, Default)]
pub struct Sources {
    /// `(prefix, source)`, longest prefix first so a nested include root wins
    /// over the one that contains it.
    entries: Vec<(String, Source)>,
}

impl Sources {
    pub fn for_scope(scope: &Scope) -> Self {
        let mut entries: Vec<(String, Source)> = scope
            .include_roots()
            .iter()
            .filter(|root| root.present)
            .map(|root| {
                // An unreadable or malformed manifest is not an error here. The
                // reader did not write it, cannot fix it, and is entitled to a
                // search result either way — they just get the directory name.
                let manifest = Config::load(&root.path).unwrap_or_default().vault;
                let fallback = root
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| root.declared.clone());
                let source = Source {
                    root: root.declared.clone(),
                    name: manifest.name.unwrap_or(fallback),
                    license: manifest.license,
                    version: manifest.version,
                };
                (root.prefix.clone(), source)
            })
            .collect();
        entries.sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));
        Self { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The source of a note key, or `None` when the note is the reader's own.
    pub fn of(&self, key: &str) -> Option<&Source> {
        self.entries
            .iter()
            .find(|(prefix, _)| {
                key == prefix
                    // Prefix match on the path separator, not on the string:
                    // `vendor/handbook-notes/x.md` does not belong to
                    // `vendor/handbook`.
                    || (key.starts_with(prefix) && key.as_bytes().get(prefix.len()) == Some(&b'/'))
            })
            .map(|(_, source)| source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::CONFIG_FILE;

    fn vault_with_installed(manifest: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CONFIG_FILE),
            "[scope]\ninclude = [\"vendor/handbook\"]\n",
        )
        .unwrap();
        let installed = dir.path().join("vendor/handbook");
        std::fs::create_dir_all(&installed).unwrap();
        if let Some(manifest) = manifest {
            std::fs::write(installed.join(CONFIG_FILE), manifest).unwrap();
        }
        dir
    }

    #[test]
    fn a_hit_from_an_installed_vault_carries_its_name_and_licence() {
        let dir = vault_with_installed(Some(
            "[vault]\nname = \"SRE Handbook\"\nlicense = \"CC-BY-4.0\"\nversion = \"2.1.0\"\n",
        ));
        let sources = Sources::for_scope(&Scope::load(dir.path()).unwrap());

        let source = sources
            .of("vendor/handbook/Runbook.md")
            .expect("has a source");
        assert_eq!(source.name, "SRE Handbook");
        assert_eq!(source.license.as_deref(), Some("CC-BY-4.0"));
        assert_eq!(source.label(), "SRE Handbook · CC-BY-4.0");
    }

    #[test]
    fn the_readers_own_notes_have_no_source() {
        let dir = vault_with_installed(None);
        let sources = Sources::for_scope(&Scope::load(dir.path()).unwrap());
        assert!(sources.of("Mine.md").is_none());
        assert!(sources.of("notes/Mine.md").is_none());
    }

    /// A vault that says nothing about itself still has to be attributable —
    /// the directory name is worse than a real name but far better than silence.
    #[test]
    fn a_source_with_no_manifest_falls_back_to_its_directory_name() {
        let dir = vault_with_installed(None);
        let sources = Sources::for_scope(&Scope::load(dir.path()).unwrap());
        let source = sources.of("vendor/handbook/Runbook.md").unwrap();
        assert_eq!(source.name, "handbook");
        assert_eq!(source.label(), "handbook · licence not stated");
    }

    /// The prefix rule has to be the same one `Scope::is_reference` uses, or a
    /// note could be read-only under one and unattributed under the other.
    #[test]
    fn a_sibling_directory_with_a_shared_prefix_is_not_the_same_source() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CONFIG_FILE),
            "[scope]\ninclude = [\"vendor/handbook\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("vendor/handbook")).unwrap();
        std::fs::create_dir_all(dir.path().join("vendor/handbook-notes")).unwrap();

        let scope = Scope::load(dir.path()).unwrap();
        let sources = Sources::for_scope(&scope);
        assert!(sources.of("vendor/handbook-notes/x.md").is_none());
        assert!(!scope.is_reference("vendor/handbook-notes/x.md"));
    }
}
