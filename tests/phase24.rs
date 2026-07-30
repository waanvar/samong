//! Phase 24 — search ranks by relevance *and* connectedness.
//!
//! Lexical relevance alone cannot tell apart six notes that use the same word the
//! same number of times, and in a real vault they are not equally useful: the one
//! the rest of your notes point at usually is. The graph already knows which one
//! that is, and the graph view already draws it bigger; this makes search agree.
//!
//! Verified through the CLI rather than the library, because the claim is that
//! every front-end shares one ranked entry point — a unit test on `search` would
//! pass even if the CLI still called the unranked function.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

/// Own registry per invocation — see the note in phase1.rs.
fn samong(cwd: &Path) -> Command {
    let mut cmd = Command::cargo_bin("samong").expect("binary should build");
    cmd.env("SAMONG_CONFIG_DIR", cwd.join(".samong-test-config"))
        .current_dir(cwd);
    cmd
}

fn write(vault: &Path, name: &str, body: &str) {
    fs::write(vault.join(format!("{name}.md")), body).unwrap();
}

/// Six candidates the query cannot distinguish, and five notes pointing at one of
/// them. The linkers deliberately do not contain the search term, so they change
/// the graph without changing anyone's relevance.
fn vault_with_one_well_linked_note() -> tempfile::TempDir {
    let vault = tempfile::tempdir().unwrap();
    for name in ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"] {
        write(
            vault.path(),
            name,
            &format!("# {name}\n\nidentical body mentioning keyword once\n"),
        );
    }
    for i in 1..=5 {
        write(
            vault.path(),
            &format!("Pointer{i}"),
            &format!("# Pointer{i}\n\nsee [[Zeta]] for the details\n"),
        );
    }
    vault
}

#[test]
fn the_note_everything_links_to_wins_a_tie() {
    let vault = vault_with_one_well_linked_note();

    samong(vault.path())
        .args(["search", "keyword", "--limit", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Zeta.md"));
}

/// Ranking must not quietly widen the result set: the pool it fetches internally
/// is larger than the limit, and only the limit may come back.
#[test]
fn ranking_respects_the_limit() {
    let vault = vault_with_one_well_linked_note();

    let output = samong(vault.path())
        .args(["search", "keyword", "--limit", "2"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    let lines = text.lines().filter(|l| l.contains(".md:")).count();
    assert_eq!(lines, 2, "expected exactly 2 hits, got:\n{text}");
}

/// The other half of the contract. A note that plainly matches the words must
/// beat a well-connected note that barely does, or search becomes a popularity
/// contest and the feature is worse than what it replaced.
#[test]
fn a_strong_match_beats_a_popular_weak_one() {
    let vault = tempfile::tempdir().unwrap();
    write(
        vault.path(),
        "Strong",
        "# Strong\n\nkeyword keyword keyword keyword keyword keyword\n",
    );
    write(
        vault.path(),
        "Popular",
        &format!("# Popular\n\nkeyword {}\n", "unrelated filler ".repeat(30)),
    );
    for i in 1..=20 {
        write(
            vault.path(),
            &format!("Fan{i}"),
            &format!("# Fan{i}\n\nlinks [[Popular]]\n"),
        );
    }

    let output = samong(vault.path())
        .args(["search", "keyword", "--limit", "2"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    let first = text.lines().next().unwrap_or_default();
    assert!(
        first.starts_with("Strong.md:"),
        "relevance must still lead; got:\n{text}"
    );
}
