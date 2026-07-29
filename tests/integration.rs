use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

/// Every invocation gets its own registry, inside the vault's own temp dir.
/// Tests run in parallel and redb takes an exclusive lock, so sharing one
/// registry makes them collide — which is exactly how CI failed while this
/// passed locally. Pointing at the real `~/.config/samong` would also let a test
/// mutate the registry someone actually uses. The dir is a dot-dir, so the scope
/// walker never counts it as notes.
fn samong(cwd: &Path) -> Command {
    let mut cmd = Command::cargo_bin("samong").expect("binary should build");
    cmd.env("SAMONG_CONFIG_DIR", cwd.join(".samong-test-config"))
        .current_dir(cwd);
    cmd
}

#[test]
fn full_lifecycle_new_links_search_graph_list() {
    let vault = tempfile::tempdir().unwrap();

    samong(vault.path()).args(["new", "A"]).assert().success();

    samong(vault.path()).args(["new", "B"]).assert().success();

    // Give both notes distinctive content, and make B link to A.
    fs::write(
        vault.path().join("A.md"),
        "# A\n\nQuantum computing uses superposition.\n",
    )
    .unwrap();
    fs::write(vault.path().join("B.md"), "# B\n\nSee [[A]] for details.\n").unwrap();

    samong(vault.path())
        .arg("reindex")
        .assert()
        .success()
        .stdout(predicate::str::contains("reindex complete"));

    // links: A should show a backlink from B
    samong(vault.path())
        .args(["links", "A"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<- B"));

    // search: content unique to A should be found, labelled with its path
    samong(vault.path())
        .args(["search", "superposition"])
        .assert()
        .success()
        .stdout(predicate::str::contains("A.md:"));

    // graph: the B -> A edge should be listed
    samong(vault.path())
        .arg("graph")
        .assert()
        .success()
        .stdout(predicate::str::contains("B -> A"));

    // list: both notes should be present
    let list_output = samong(vault.path())
        .arg("list")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_text = String::from_utf8(list_output).unwrap();
    assert!(list_text.contains('A'));
    assert!(list_text.contains('B'));
}

#[test]
fn new_rejects_duplicate_title() {
    let vault = tempfile::tempdir().unwrap();

    samong(vault.path()).args(["new", "Dup"]).assert().success();

    samong(vault.path()).args(["new", "Dup"]).assert().failure();
}

#[test]
fn search_with_no_notes_reports_no_results() {
    let vault = tempfile::tempdir().unwrap();

    samong(vault.path()).arg("reindex").assert().success();

    samong(vault.path())
        .args(["search", "nothing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no results"));
}
