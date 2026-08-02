//! Phase 28 — installing someone else's vault, end to end against real git.
//!
//! The unit tests in `install.rs` cover the file edits. These drive the actual
//! commands against a real local repository, because the parts most likely to
//! break are the ones the unit tests cannot see: whether the clone lands where
//! `scope.include` can reach it, whether the notes come back as *read-only*, and
//! whether `update` picks up a new commit.
//!
//! A local repository stands in for a paid one. The only difference in the real
//! case is that `git` is refused at the door, which is git's job, not ours.

use std::fs;
use std::path::Path;
use std::process::Command as Proc;

use assert_cmd::Command;
use predicates::prelude::*;

fn samong(cwd: &Path) -> Command {
    let mut cmd = Command::cargo_bin("samong").expect("binary should build");
    cmd.env("SAMONG_CONFIG_DIR", cwd.join(".samong-test-config"))
        .current_dir(cwd);
    cmd
}

fn git(cwd: &Path, args: &[&str]) {
    let out = Proc::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .expect("git should be installed to run these tests");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A vault published as a git repository, the way a seller would leave it.
fn published_vault(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("samong.toml"),
        "[vault]\nname = \"handbook\"\ndescription = \"Ops knowledge\"\n\
         version = \"1.0.0\"\nlicense = \"CC-BY-4.0\"\n",
    )
    .unwrap();
    fs::write(dir.join("Runbook.md"), "# Runbook\n\nrestart the thing\n").unwrap();
    git(dir, &["init", "--quiet", "-b", "main"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "--quiet", "-m", "v1.0.0"]);
}

#[test]
fn install_clones_wires_scope_and_protects_the_buyer_from_committing_it() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());

    let buyer = tempfile::tempdir().unwrap();
    fs::write(buyer.path().join("Mine.md"), "# Mine\n\nSee [[Runbook]].\n").unwrap();
    fs::write(buyer.path().join(".gitignore"), "target/\n").unwrap();

    samong(buyer.path())
        .args(["vault", "install"])
        .arg(seller.path().to_str().unwrap())
        .arg("--name")
        .arg("handbook")
        .assert()
        .success()
        .stdout(
            // The buyer is told what they now hold and under what terms.
            predicate::str::contains("Ops knowledge")
                .and(predicate::str::contains("version 1.0.0"))
                .and(predicate::str::contains("licence: CC-BY-4.0"))
                .and(predicate::str::contains(
                    "added vendor/handbook to scope.include",
                ))
                .and(predicate::str::contains("not yours to commit")),
        );

    assert!(buyer.path().join("vendor/handbook/Runbook.md").exists());

    let config = fs::read_to_string(buyer.path().join("samong.toml")).unwrap();
    assert!(
        config.contains("vendor/handbook"),
        "wired into scope: {config}"
    );

    let ignored = fs::read_to_string(buyer.path().join(".gitignore")).unwrap();
    assert!(ignored.starts_with("target/\n"), "existing rules survive");
    assert!(ignored.contains("/vendor/handbook/"));
}

/// The installed notes have to be part of the same brain — findable and linkable
/// — while staying somebody else's.
#[test]
fn installed_notes_are_searchable_linkable_and_refuse_to_be_edited() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    let buyer = tempfile::tempdir().unwrap();
    fs::write(buyer.path().join("Mine.md"), "# Mine\n\nSee [[Runbook]].\n").unwrap();

    samong(buyer.path())
        .args(["vault", "install"])
        .arg(seller.path().to_str().unwrap())
        .arg("--name")
        .arg("handbook")
        .assert()
        .success();

    // Same search space.
    samong(buyer.path())
        .args(["search", "restart", "--limit", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("vendor/handbook/Runbook.md"));

    // Same link space: the buyer's [[Runbook]] resolves into the installed vault.
    samong(buyer.path())
        .arg("broken")
        .assert()
        .success()
        .stdout(predicate::str::contains("no broken links"));

    // But not the buyer's to change — an edit would be erased by the next update.
    samong(buyer.path())
        .args(["delete", "Runbook"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("read-only reference note"));
}

#[test]
fn update_pulls_new_content_and_says_what_moved() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    let buyer = tempfile::tempdir().unwrap();
    fs::write(buyer.path().join("Mine.md"), "# Mine\n").unwrap();

    samong(buyer.path())
        .args(["vault", "install"])
        .arg(seller.path().to_str().unwrap())
        .arg("--name")
        .arg("handbook")
        .assert()
        .success();

    samong(buyer.path())
        .args(["vault", "update"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already current"));

    // The seller ships an update — the thing a subscription actually buys.
    fs::write(
        seller.path().join("Escalation.md"),
        "# Escalation\n\nwake the on-call\n",
    )
    .unwrap();
    git(seller.path(), &["add", "-A"]);
    git(seller.path(), &["commit", "--quiet", "-m", "v1.1.0"]);

    samong(buyer.path())
        .args(["vault", "update"])
        .assert()
        .success()
        .stdout(predicate::str::contains("handbook: "));

    assert!(buyer.path().join("vendor/handbook/Escalation.md").exists());
    samong(buyer.path())
        .args(["search", "on-call", "--limit", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Escalation.md"));
}

#[test]
fn update_says_something_useful_when_nothing_is_installed() {
    let buyer = tempfile::tempdir().unwrap();
    fs::write(buyer.path().join("Mine.md"), "# Mine\n").unwrap();
    samong(buyer.path())
        .args(["vault", "update"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no installed vaults"));
}

#[test]
fn installing_twice_points_at_update_instead_of_clobbering() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    let buyer = tempfile::tempdir().unwrap();
    fs::write(buyer.path().join("Mine.md"), "# Mine\n").unwrap();

    let url = seller.path().to_str().unwrap().to_string();
    samong(buyer.path())
        .args(["vault", "install"])
        .arg(&url)
        .args(["--name", "handbook"])
        .assert()
        .success();
    samong(buyer.path())
        .args(["vault", "install"])
        .arg(&url)
        .args(["--name", "handbook"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("samong vault update handbook"));
}
