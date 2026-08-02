//! Phase 27 — a vault you can hand to someone else.
//!
//! `samong pack` exists because the obvious way to share a vault — zip the folder
//! — ships more than the notes. `.brain/` holds a full second copy of every note's
//! text, and the titles of deleted notes survive in `graph.redb` until their pages
//! are reused, so a seller who cleaned up before publishing would publish the
//! cleanup as well.
//!
//! These tests are the guarantee. Each one is a thing that must never end up in a
//! published copy.

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

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

/// A vault with a licence, a note in a subdirectory, and a built index.
fn packable_vault() -> tempfile::TempDir {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    write(
        root,
        "samong.toml",
        "[vault]\nname = \"handbook\"\nversion = \"1.2.0\"\nlicense = \"CC-BY-4.0\"\n",
    );
    write(root, "Index.md", "# Index\n\nSee [[Deep note]].\n");
    write(
        root,
        "topics/Deep note.md",
        "# Deep note\n\nIn a subfolder.\n",
    );
    vault
}

#[test]
fn pack_copies_notes_and_the_manifest_but_never_the_index() {
    let vault = packable_vault();
    let out = tempfile::tempdir().unwrap();
    let dest = out.path().join("published");

    // Build an index first, so there is something that could leak.
    samong(vault.path()).arg("reindex").assert().success();
    assert!(vault.path().join(".brain").exists(), "index should exist");

    samong(vault.path())
        .arg("pack")
        .arg(&dest)
        .assert()
        .success()
        .stdout(predicate::str::contains("packed 2 note(s)"));

    assert!(dest.join("Index.md").exists());
    assert!(
        dest.join("topics/Deep note.md").exists(),
        "subdirectory structure must survive"
    );
    assert!(
        dest.join("samong.toml").exists(),
        "the manifest travels too"
    );
    assert!(
        !dest.join(".brain").exists(),
        ".brain/ must never be copied: it holds a second copy of every note"
    );
}

/// The specific leak that motivated the command: a note deleted before publishing
/// leaves its title inside graph.redb, and a folder copy would carry it along.
#[test]
fn a_deleted_notes_title_does_not_reach_the_packed_copy() {
    let vault = packable_vault();
    let out = tempfile::tempdir().unwrap();
    let dest = out.path().join("published");

    write(
        vault.path(),
        "Salaries 2026.md",
        "# Salaries 2026\n\nprivate\n",
    );
    samong(vault.path()).arg("reindex").assert().success();
    fs::remove_file(vault.path().join("Salaries 2026.md")).unwrap();
    samong(vault.path()).arg("reindex").assert().success();

    // The index still carries the title even though the note is gone — that is
    // exactly why packing cannot be "copy the folder, minus a blocklist".
    let index_bytes = fs::read(vault.path().join(".brain/graph.redb")).unwrap();
    let lingers = index_bytes
        .windows(b"Salaries 2026".len())
        .any(|w| w == b"Salaries 2026");
    assert!(
        lingers,
        "precondition: the title should still be in the index"
    );

    samong(vault.path())
        .arg("pack")
        .arg(&dest)
        .assert()
        .success();

    for entry in walk(&dest) {
        let bytes = fs::read(&entry).unwrap();
        assert!(
            !bytes
                .windows(b"Salaries 2026".len())
                .any(|w| w == b"Salaries 2026"),
            "the deleted note leaked into {}",
            entry.display()
        );
    }
}

/// Publishing without saying what people may do with the notes is the hardest
/// mistake to walk back, so it is refused rather than warned about.
#[test]
fn pack_refuses_a_vault_with_no_content_licence() {
    let vault = tempfile::tempdir().unwrap();
    write(
        vault.path(),
        "samong.toml",
        "[vault]\nname = \"nolicence\"\n",
    );
    write(vault.path(), "A.md", "# A\n");
    let out = tempfile::tempdir().unwrap();

    samong(vault.path())
        .arg("pack")
        .arg(out.path().join("published"))
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("license")
                .and(predicate::str::contains("All rights reserved")),
        );
}

/// Reference notes are somebody else's documentation. Redistributing them is the
/// seller's decision to make explicitly, not a default.
#[test]
fn reference_notes_are_left_out_unless_asked_for() {
    let vault = packable_vault();
    let root = vault.path();
    fs::write(
        root.join("samong.toml"),
        "[vault]\nname = \"handbook\"\nlicense = \"CC-BY-4.0\"\n\n\
         [scope]\ninclude = [\"vendor/docs\"]\n",
    )
    .unwrap();
    write(
        root,
        "vendor/docs/Their guide.md",
        "# Their guide\n\nnot mine\n",
    );

    let out = tempfile::tempdir().unwrap();
    let dest = out.path().join("published");
    samong(root)
        .arg("pack")
        .arg(&dest)
        .assert()
        .success()
        .stdout(predicate::str::contains("left out 1 reference note"));
    assert!(!dest.join("vendor/docs/Their guide.md").exists());

    let with_ref = out.path().join("with-reference");
    samong(root)
        .arg("pack")
        .arg(&with_ref)
        .arg("--include-reference")
        .assert()
        .success()
        .stdout(predicate::str::contains("redistributing someone else's"));
    assert!(with_ref.join("vendor/docs/Their guide.md").exists());
}

#[test]
fn pack_will_not_write_into_a_directory_that_already_has_files() {
    let vault = packable_vault();
    let out = tempfile::tempdir().unwrap();
    fs::write(out.path().join("something.txt"), "already here").unwrap();

    samong(vault.path())
        .arg("pack")
        .arg(out.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already has files"));
}

/// The manifest fields existed unread since Phase 10. Reporting them is what makes
/// a vault able to introduce itself once it leaves the machine it was written on.
#[test]
fn doctor_reports_what_the_vault_says_about_itself() {
    let vault = packable_vault();
    fs::write(
        vault.path().join("samong.toml"),
        "[vault]\nname = \"handbook\"\ndescription = \"Ops knowledge\"\n\
         version = \"1.2.0\"\nlicense = \"CC-BY-4.0\"\nsource = \"https://example.com/x.git\"\n",
    )
    .unwrap();

    samong(vault.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("about: Ops knowledge")
                .and(predicate::str::contains("content version: 1.2.0"))
                .and(predicate::str::contains("content licence: CC-BY-4.0"))
                .and(predicate::str::contains(
                    "source: https://example.com/x.git",
                )),
        );
}

/// Only nag about a missing licence when the vault already looks like it travels.
#[test]
fn doctor_flags_a_missing_licence_only_on_a_vault_that_declares_a_source() {
    let quiet = tempfile::tempdir().unwrap();
    write(quiet.path(), "samong.toml", "[vault]\nname = \"private\"\n");
    write(quiet.path(), "A.md", "# A\n");
    samong(quiet.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("content licence").not());

    let shared = tempfile::tempdir().unwrap();
    write(
        shared.path(),
        "samong.toml",
        "[vault]\nname = \"shared\"\nsource = \"https://example.com/x.git\"\n",
    );
    write(shared.path(), "A.md", "# A\n");
    samong(shared.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("licence: not set"));
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}
