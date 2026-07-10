use std::fs;
use std::path::Path;
use std::time::Instant;

use assert_cmd::Command;
use predicates::prelude::*;

fn banyan() -> Command {
    Command::cargo_bin("banyan").expect("binary should build")
}

fn write_note(vault: &Path, title: &str, body: &str) {
    fs::write(vault.join(format!("{title}.md")), body).unwrap();
}

/// Acceptance: renaming a note rewrites every [[wikilink]] pointing at it.
#[test]
fn rename_rewrites_links_in_every_referencing_note() {
    let vault = tempfile::tempdir().unwrap();
    write_note(vault.path(), "A", "# A\n\ntarget note\n");
    write_note(vault.path(), "B", "# B\n\nplain link [[A]] here\n");
    write_note(vault.path(), "C", "# C\n\naliased [[A|the A note]] link\n");
    write_note(vault.path(), "D", "# D\n\nno links at all\n");

    banyan()
        .current_dir(vault.path())
        .args(["rename", "A", "Z"])
        .assert()
        .success()
        .stdout(predicate::str::contains("updated 2 link(s) in 2 note(s)"));

    // The file moved...
    assert!(!vault.path().join("A.md").exists());
    assert!(vault.path().join("Z.md").exists());

    // ...and every referencing note was rewritten, alias preserved.
    let b = fs::read_to_string(vault.path().join("B.md")).unwrap();
    assert!(b.contains("[[Z]]"), "B.md should point at Z: {b}");
    let c = fs::read_to_string(vault.path().join("C.md")).unwrap();
    assert!(c.contains("[[Z|the A note]]"), "alias must survive: {c}");
    let d = fs::read_to_string(vault.path().join("D.md")).unwrap();
    assert!(!d.contains("[[Z]]"), "unrelated note must be untouched");

    // The graph agrees: Z has both backlinks, A is gone entirely.
    banyan()
        .current_dir(vault.path())
        .args(["links", "Z"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<- B").and(predicate::str::contains("<- C")));
    banyan()
        .current_dir(vault.path())
        .args(["links", "A"])
        .assert()
        .success()
        .stdout(predicate::str::contains("backlinks (0)"));
}

#[test]
fn rename_rejects_existing_target_and_missing_source() {
    let vault = tempfile::tempdir().unwrap();
    write_note(vault.path(), "A", "# A\n");
    write_note(vault.path(), "B", "# B\n");

    banyan()
        .current_dir(vault.path())
        .args(["rename", "A", "B"])
        .assert()
        .failure();
    banyan()
        .current_dir(vault.path())
        .args(["rename", "Missing", "X"])
        .assert()
        .failure();
}

#[test]
fn delete_removes_note_and_reports_dangling_backlinks() {
    let vault = tempfile::tempdir().unwrap();
    write_note(vault.path(), "A", "# A\n\nwill be deleted\n");
    write_note(vault.path(), "B", "# B\n\nstill points at [[A]]\n");

    banyan()
        .current_dir(vault.path())
        .args(["delete", "A"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("deleted \"A\"").and(predicate::str::contains("B -> [[A]]")),
        );

    assert!(!vault.path().join("A.md").exists());

    // The dangling link now shows up in `broken`.
    banyan()
        .current_dir(vault.path())
        .arg("broken")
        .assert()
        .success()
        .stdout(predicate::str::contains("B -> [[A]]"));

    // And the deleted note no longer matches searches.
    banyan()
        .current_dir(vault.path())
        .args(["search", "deleted"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no results"));
}

#[test]
fn orphans_lists_only_unlinked_notes() {
    let vault = tempfile::tempdir().unwrap();
    write_note(vault.path(), "Hub", "# Hub\n\nlinks [[Leaf]]\n");
    write_note(vault.path(), "Leaf", "# Leaf\n");
    write_note(vault.path(), "Loner", "# Loner\n");

    let output = banyan()
        .current_dir(vault.path())
        .arg("orphans")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Hub"), "Hub has no backlinks: {text}");
    assert!(text.contains("Loner"), "Loner has no backlinks: {text}");
    assert!(!text.contains("Leaf"), "Leaf is linked from Hub: {text}");
}

#[test]
fn broken_reports_nothing_for_healthy_vault() {
    let vault = tempfile::tempdir().unwrap();
    write_note(vault.path(), "A", "# A\n\n[[B]]\n");
    write_note(vault.path(), "B", "# B\n");

    banyan()
        .current_dir(vault.path())
        .arg("broken")
        .assert()
        .success()
        .stdout(predicate::str::contains("no broken links"));
}

#[test]
fn edit_runs_editor_and_reindexes() {
    let vault = tempfile::tempdir().unwrap();
    write_note(vault.path(), "A", "# A\n");

    // A no-op "editor" that exits 0 without touching the file.
    let editor = if cfg!(windows) { "cmd /C rem" } else { "true" };
    banyan()
        .current_dir(vault.path())
        .env("EDITOR", editor)
        .args(["edit", "A"])
        .assert()
        .success()
        .stdout(predicate::str::contains("reindexed"));

    banyan()
        .current_dir(vault.path())
        .env("EDITOR", editor)
        .args(["edit", "Missing"])
        .assert()
        .failure();
}

/// Acceptance: with a 1,000-note vault and a single changed file, incremental
/// reindex must be clearly faster than a full reindex.
#[test]
fn incremental_reindex_beats_full_reindex_on_large_vault() {
    let vault = tempfile::tempdir().unwrap();
    for i in 0..1000 {
        write_note(
            vault.path(),
            &format!("note-{i:04}"),
            &format!(
                "# note-{i:04}\n\nbody text {i} links [[note-{:04}]]\n",
                (i + 1) % 1000
            ),
        );
    }

    let full_start = Instant::now();
    banyan()
        .current_dir(vault.path())
        .args(["reindex", "--full"])
        .assert()
        .success()
        .stdout(predicate::str::contains("reindexed 1000 note(s) (full)"));
    let full_elapsed = full_start.elapsed();

    // Touch exactly one note.
    write_note(vault.path(), "note-0500", "# note-0500\n\nedited body\n");

    let inc_start = Instant::now();
    banyan()
        .current_dir(vault.path())
        .arg("reindex")
        .assert()
        .success()
        .stdout(predicate::str::contains("reindexed 1 note(s), removed 0"));
    let inc_elapsed = inc_start.elapsed();

    assert!(
        inc_elapsed < full_elapsed / 2,
        "incremental ({inc_elapsed:?}) must be clearly faster than full ({full_elapsed:?})"
    );
}
