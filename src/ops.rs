//! Small operations shared by every front-end (CLI, HTTP API, MCP) so the
//! behavior stays identical no matter which surface an agent or human uses.

use anyhow::Result;

use crate::graph::{self, Graph};
use crate::registry::Registry;

/// Collapse note keys into display titles: sorted and de-duplicated.
///
/// Notes are identified and addressed by path, because titles are not unique.
/// Titles remain what `[[wikilinks]]` name and what a person reads, so this is
/// for *display* only — anywhere a caller needs to act on the result, it should
/// be handed keys instead.
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

/// Refuse to modify a reference note, explaining what to do instead.
///
/// Reference notes come from `scope.include` — vendored documentation and the
/// like. Writing there is worse than useless: the file belongs to a dependency,
/// so the next install silently erases the change, taking whatever was just
/// learned with it.
///
/// Every write path funnels through here — CLI delete/rename, HTTP PUT/DELETE,
/// MCP save_note — so the refusal reads the same everywhere and there is exactly
/// one place to get the wording right.
pub fn reject_reference_write(scope: &crate::scope::Scope, key: &str, action: &str) -> Result<()> {
    if !scope.is_reference(key) {
        return Ok(());
    }
    anyhow::bail!(
        "cannot {action} \"{key}\": it is a read-only reference note from a scope.include \
         directory (it belongs to a dependency and would be erased on reinstall)"
    )
}

/// Reciprocal Rank Fusion constant, from the paper the method comes from.
///
/// Large enough that the top few results do not dwarf everything below them, so a
/// note ranked second by both rankings can beat one ranked first by only one.
const RRF_K: f32 = 60.0;

/// Search one vault: lexical relevance, connectedness, and — when the vault has
/// embeddings and this build has them compiled in — meaning.
///
/// The single entry point for every front-end, so a query typed in the terminal,
/// sent to the HTTP API and asked by an agent all come back in the same order.
/// Composing the indexes here rather than inside `search` keeps that module about
/// full text and nothing else.
///
/// A vault with no graph yet — or a graph that cannot be opened, which is a
/// stale-index problem, not a search problem — degrades to plain relevance rather
/// than failing the query. A vault nobody has embedded stays purely lexical.
///
/// # Why the score is a fused rank, always
///
/// `SearchHit::score` comes back on the RRF scale even when there is only one
/// ranking to fuse. Callers that merge several vaults sort on this field, and if
/// one vault answered in BM25 units and another in fused units the merge would be
/// meaningless. Fusing a single ranking preserves its order exactly, so the
/// lexical-only case is unchanged in everything but the numbers.
pub fn search_vault(
    vault: &std::path::Path,
    query: &str,
    options: &crate::search::SearchOptions,
) -> Result<Vec<crate::search::SearchHit>> {
    let degrees = Graph::open(vault)
        .and_then(|graph| graph.degrees())
        .unwrap_or_default();
    // Degree is folded into the lexical score here, not into the fused score:
    // fused scores sit within a percent or two of each other, where the 25% boost
    // would leap many positions instead of settling near-ties. Connectedness gets
    // one bite, on the ranking whose gaps it was tuned against.
    let mut hits = crate::search::query_ranked(vault, query, options, &degrees)?;

    let semantic_order = semantic_ranking(vault, query, options);
    if semantic_order.is_empty() {
        // Nothing to fuse with; still emit fused-scale scores.
        for (index, hit) in hits.iter_mut().enumerate() {
            hit.score = 1.0 / (RRF_K + index as f32 + 1.0);
        }
        return Ok(hits);
    }

    let lexical_rank: std::collections::HashMap<String, usize> = hits
        .iter()
        .enumerate()
        .map(|(index, hit)| (hit.key.clone(), index))
        .collect();

    // Candidates the words missed entirely.
    //
    // This is the whole point, and the first version of this function got it
    // wrong: it started from the lexical hits and let meaning only reorder them,
    // so a query whose words appear nowhere returned nothing at all — proved by
    // asking a Thai question of an English vault and getting "no results" with
    // embeddings switched on. Hybrid retrieval has to be a union of the two
    // candidate sets, not one filtered by the other.
    let unmatched: Vec<String> = semantic_order
        .iter()
        .filter(|key| !lexical_rank.contains_key(*key))
        .cloned()
        .collect();
    hits.extend(crate::search::hits_for_keys(
        vault,
        &unmatched,
        options.snippet_chars,
    )?);

    let semantic_rank: std::collections::HashMap<&str, usize> = semantic_order
        .iter()
        .enumerate()
        .map(|(index, key)| (key.as_str(), index))
        .collect();
    for hit in hits.iter_mut() {
        // Missing from one ranking contributes nothing from that side rather than
        // a penalty: being found by one of two methods is not evidence against.
        let lexical = lexical_rank
            .get(&hit.key)
            .map(|rank| 1.0 / (RRF_K + *rank as f32 + 1.0))
            .unwrap_or(0.0);
        let meaning = semantic_rank
            .get(hit.key.as_str())
            .map(|rank| 1.0 / (RRF_K + *rank as f32 + 1.0))
            .unwrap_or(0.0);
        hit.score = lexical + meaning;
    }
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.key.cmp(&b.key))
    });
    // The union is larger than what was asked for.
    hits.truncate(options.limit.clamp(1, crate::search::MAX_LIMIT));
    Ok(hits)
}

/// Keys ordered by meaning, or empty when semantic search is not available here.
///
/// Fails soft on purpose: a missing model download or a half-written vector store
/// must degrade the ranking, never break the query. Search is the one thing that
/// has to keep working.
#[cfg(feature = "semantic")]
fn semantic_ranking(
    vault: &std::path::Path,
    query: &str,
    options: &crate::search::SearchOptions,
) -> Vec<String> {
    if !crate::vectors::exists(vault) {
        return Vec::new();
    }
    match crate::semantic::rank_by_similarity(vault, query, options.limit * 3) {
        Ok(order) => order,
        Err(error) => {
            eprintln!("warning: semantic ranking unavailable, using words only: {error}");
            Vec::new()
        }
    }
}

#[cfg(not(feature = "semantic"))]
fn semantic_ranking(
    _vault: &std::path::Path,
    _query: &str,
    _options: &crate::search::SearchOptions,
) -> Vec<String> {
    Vec::new()
}

/// Note keys arrive from untrusted callers — URL segments, MCP tool arguments —
/// and unlike a bare title they are *supposed* to contain slashes, so every
/// other way of escaping a vault has to be closed explicitly.
///
/// A valid key is what [`crate::vault::relative_key`] produces: a relative,
/// slash-separated path to a `.md` file, with no `.`/`..` components and no
/// backslashes (keeping one canonical spelling means the same note cannot arrive
/// under two different keys).
pub fn validate_key(key: &str) -> std::result::Result<(), String> {
    let invalid = |why: &str| Err(format!("invalid note path \"{key}\": {why}"));

    if key.is_empty() {
        return invalid("empty");
    }
    if key.contains('\\') {
        return invalid("use forward slashes");
    }
    if key.contains('\0') {
        return invalid("contains a null byte");
    }
    if key.starts_with('/') {
        return invalid("must be relative to the vault");
    }
    // A Windows drive or UNC prefix would otherwise survive `Path::join`.
    if key.chars().nth(1) == Some(':') {
        return invalid("must be relative to the vault");
    }
    if !key.ends_with(".md") {
        return invalid("notes are .md files");
    }
    for component in key.split('/') {
        match component {
            "" => return invalid("has an empty path segment"),
            "." | ".." => return invalid("must not contain \".\" or \"..\""),
            _ => {}
        }
    }
    Ok(())
}

/// Resolve a validated key against a vault root, refusing anything that still
/// lands outside it.
///
/// [`validate_key`] already rejects the obvious escapes; this is the second
/// line, catching the cases a string check cannot see — a symlinked directory
/// inside the vault pointing elsewhere, most of all.
pub fn resolve_key(
    vault: &std::path::Path,
    key: &str,
) -> std::result::Result<std::path::PathBuf, String> {
    validate_key(key)?;
    let candidate = vault.join(key);

    // The file may not exist yet (creating a note), so canonicalize the deepest
    // existing ancestor instead and check that.
    let mut existing = candidate.as_path();
    let anchor = loop {
        match existing.parent() {
            Some(parent) => {
                if parent.exists() {
                    break parent;
                }
                existing = parent;
            }
            None => return Err(format!("invalid note path \"{key}\"")),
        }
    };
    let (Ok(real_anchor), Ok(real_vault)) = (anchor.canonicalize(), vault.canonicalize()) else {
        return Err(format!("cannot resolve note path \"{key}\""));
    };
    if !real_anchor.starts_with(&real_vault) {
        return Err(format!("note path \"{key}\" resolves outside the vault"));
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_key_accepts_real_keys() {
        for ok in [
            "Note.md",
            "docs/API.md",
            "a/b/c/Deep Note.md",
            "โน้ตไทย.md",
            "node_modules/next/dist/docs/01-app/installation.md",
        ] {
            assert!(validate_key(ok).is_ok(), "{ok:?} should be accepted");
        }
    }

    #[test]
    fn validate_key_blocks_every_escape() {
        for bad in [
            "",                   // empty
            "Note",               // not a .md file
            "notes/",             // trailing slash
            "/etc/passwd.md",     // absolute
            "C:/Windows/evil.md", // drive prefix
            "../outside.md",      // parent escape
            "docs/../../out.md",  // parent escape mid-path
            "docs/./A.md",        // "." component
            "docs//A.md",         // empty segment
            "docs\\A.md",         // backslash: keys have one spelling
            "docs/A.md\0.txt",    // null byte
        ] {
            assert!(validate_key(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn resolve_key_stays_inside_the_vault() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        std::fs::create_dir_all(vault.join("docs")).unwrap();

        // Existing directory, and a note that does not exist yet.
        assert!(resolve_key(vault, "docs/API.md").is_ok());
        assert!(resolve_key(vault, "brand/new/Note.md").is_ok());
        // Escapes are refused even though the string looks harmless in parts.
        assert!(resolve_key(vault, "../evil.md").is_err());
    }
}
