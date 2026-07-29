use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

/// Own registry per invocation — see the note in phase1.rs: a shared redb
/// registry makes parallel tests fight over an exclusive lock.
fn samong(cwd: &Path) -> Command {
    let mut cmd = Command::cargo_bin("samong").expect("binary should build");
    cmd.env("SAMONG_CONFIG_DIR", cwd.join(".samong-test-config"))
        .current_dir(cwd);
    cmd
}

fn write_note(vault: &Path, title: &str, body: &str) {
    fs::write(vault.join(format!("{title}.md")), body).unwrap();
}

/// Acceptance: Thai words in the middle of an unspaced sentence must be
/// searchable — this is exactly what Obsidian cannot do.
#[test]
fn finds_thai_words_inside_unspaced_sentences() {
    let vault = tempfile::tempdir().unwrap();
    write_note(
        vault.path(),
        "หุ้น",
        "# หุ้น\n\nตลาดหลักทรัพย์แห่งประเทศไทยเปิดทำการวันนี้\n",
    );
    write_note(vault.path(), "อาหาร", "# อาหาร\n\nร้านข้าวมันไก่เปิดใหม่แถวบ้าน\n");
    samong(vault.path()).arg("reindex").assert().success();

    // Mid-sentence compound word, no spaces anywhere around it.
    samong(vault.path())
        .args(["search", "ตลาดหลักทรัพย์"])
        .assert()
        .success()
        .stdout(predicate::str::contains("หุ้น").and(predicate::str::contains("อาหาร").not()));

    // Suffix of the same compound phrase.
    samong(vault.path())
        .args(["search", "ประเทศไทย"])
        .assert()
        .success()
        .stdout(predicate::str::contains("หุ้น"));

    // The other note is still reachable by its own content.
    samong(vault.path())
        .args(["search", "ข้าวมันไก่"])
        .assert()
        .success()
        .stdout(predicate::str::contains("อาหาร"));
}

#[test]
fn thai_snippets_highlight_the_matched_words() {
    let vault = tempfile::tempdir().unwrap();
    write_note(
        vault.path(),
        "โน้ตไทย",
        "# โน้ตไทย\n\nระบบค้นหารองรับภาษาไทยเต็มรูปแบบ\n",
    );
    samong(vault.path()).arg("reindex").assert().success();

    samong(vault.path())
        .args(["search", "ภาษาไทย"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<b>"));
}

#[test]
fn mixed_thai_english_notes_match_both_languages() {
    let vault = tempfile::tempdir().unwrap();
    write_note(
        vault.path(),
        "Notes App",
        "# Notes App\n\nเขียนจดโน้ตด้วย Rust และ tantivy รองรับการค้นหาภาษาไทย\n",
    );
    samong(vault.path()).arg("reindex").assert().success();

    samong(vault.path())
        .args(["search", "tantivy"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Notes App"));
    samong(vault.path())
        .args(["search", "จดโน้ต"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Notes App"));
}

/// An index built with an older format version is rebuilt automatically on
/// the next command that syncs the index.
#[test]
fn stale_index_version_triggers_automatic_full_reindex() {
    let vault = tempfile::tempdir().unwrap();
    write_note(vault.path(), "A", "# A\n\nfirst body\n");
    samong(vault.path()).arg("reindex").assert().success();

    // Simulate an index produced by an older samong: nuke the recorded
    // version by wiping .brain's tantivy dir and the graph db entirely.
    fs::remove_dir_all(vault.path().join(".brain")).unwrap();

    // Any index-syncing command must silently rebuild everything.
    samong(vault.path())
        .arg("reindex")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("(full)")
                .and(predicate::str::contains("index format changed")),
        );

    samong(vault.path())
        .args(["search", "first"])
        .assert()
        .success()
        .stdout(predicate::str::contains("A"));
}
