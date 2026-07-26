//! Small operations shared by every front-end (CLI, HTTP API, MCP) so the
//! behavior stays identical no matter which surface an agent or human uses.

use anyhow::Result;

use crate::graph::{self, Graph};
use crate::registry::Registry;

/// Collapse note keys into display titles: sorted and de-duplicated.
///
/// The indexes identify notes by path, because titles are not unique. Every
/// front-end still *addresses* notes by title — that is what `[[wikilinks]]`
/// contain — so anything user-facing converts back here. The split is
/// deliberate: paths keep the stored data correct, titles keep the interface
/// the one people and agents already use.
pub fn keys_to_titles(keys: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = keys
        .iter()
        .filter_map(|key| graph::title_from_key(key))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Backlinks pointing at `vault_name/title` from every *other* registered
/// vault, formatted as `vault/note`. Query-time federation: each vault's own
/// graph already indexed the raw cross-vault target, so this is a single
/// backward lookup per vault — no cross-vault index writes anywhere.
pub fn cross_vault_backlinks(
    registry: &Registry,
    vault_name: &str,
    title: &str,
) -> Result<Vec<String>> {
    let qualified = format!("{vault_name}/{title}");
    let mut out = Vec::new();
    for (other_name, other_path) in registry.list()? {
        if other_name == vault_name {
            continue;
        }
        let sources = {
            let graph = Graph::open(&other_path)?;
            graph.backlinks(&qualified)?
        };
        out.extend(
            keys_to_titles(sources)
                .into_iter()
                .map(|title| format!("{other_name}/{title}")),
        );
    }
    Ok(out)
}

/// Note titles arrive from untrusted callers (URL segments, MCP tool
/// arguments); never let one escape the vault directory.
pub fn validate_title(title: &str) -> std::result::Result<(), String> {
    if title.is_empty()
        || title.contains(['/', '\\'])
        || title == "."
        || title == ".."
        || title.ends_with(".md")
    {
        return Err(format!("invalid note title \"{title}\""));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_title_blocks_escapes() {
        assert!(validate_title("ok note ไทย").is_ok());
        for bad in ["", ".", "..", "a/b", "a\\b", "note.md", "../evil"] {
            assert!(validate_title(bad).is_err(), "{bad:?} should be rejected");
        }
    }
}
