//! Phase 25 — semantic search is optional, and says so.
//!
//! The feature is off in every published binary, so the commonest encounter with
//! it will be someone reading the docs, typing `samong embed`, and finding out it
//! is not there. That moment has to explain itself: what is missing, why it is
//! missing, and the one command that gets it. An "unknown subcommand" error would
//! leave them wondering whether they typed it wrong.
//!
//! Everything past that point — the vector store, chunking, fusion — is verified
//! in the library tests and by the `semantic` CI job, which is the only place the
//! feature can be linked reliably.

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

#[test]
#[cfg(not(feature = "semantic"))]
fn embed_without_the_feature_explains_itself() {
    let vault = tempfile::tempdir().unwrap();
    fs::write(vault.path().join("A.md"), "# A\n\nsome content\n").unwrap();

    samong(vault.path()).arg("embed").assert().failure().stderr(
        predicate::str::contains("opt-in").and(predicate::str::contains("--features semantic")),
    );
}

/// The command has to exist in the help either way, so it is discoverable rather
/// than folklore.
#[test]
fn embed_is_listed_in_help() {
    let vault = tempfile::tempdir().unwrap();
    samong(vault.path())
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("embed"));
}

/// Search must behave identically on a vault nobody embedded, with or without the
/// feature compiled in — the fusion changes the score scale, not the answers.
#[test]
fn search_is_unchanged_on_a_vault_with_no_embeddings() {
    let vault = tempfile::tempdir().unwrap();
    fs::write(
        vault.path().join("Strong.md"),
        "# Strong\n\nkeyword keyword keyword keyword\n",
    )
    .unwrap();
    fs::write(
        vault.path().join("Weak.md"),
        "# Weak\n\nkeyword buried among plenty of other unrelated words\n",
    )
    .unwrap();

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
        "lexical order must survive the fusion rewrite; got:\n{text}"
    );
}

/// And `doctor` should not start claiming things about embeddings in a build that
/// cannot make any.
#[test]
#[cfg(not(feature = "semantic"))]
fn doctor_says_nothing_about_embeddings_without_the_feature() {
    let vault = tempfile::tempdir().unwrap();
    fs::write(vault.path().join("A.md"), "# A\n\ncontent\n").unwrap();

    samong(vault.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("embeddings").not());
}
