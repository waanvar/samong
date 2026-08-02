//! Phase 29 — proving who published a vault, and saying so where it is read.
//!
//! Two halves of one question. `vault verify` answers "is this the vault its
//! publisher published", once, deliberately. Provenance on search results
//! answers "whose work is this" every time somebody reads a result — which is
//! the moment that actually matters, because it is the moment a paragraph gets
//! copied out of a bought vault into notes of one's own.

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
    let out = git_raw(cwd, args);
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git_raw(cwd: &Path, args: &[&str]) -> std::process::Output {
    Proc::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "Publisher")
        .env("GIT_AUTHOR_EMAIL", "pub@example.com")
        .env("GIT_COMMITTER_NAME", "Publisher")
        .env("GIT_COMMITTER_EMAIL", "pub@example.com")
        .output()
        .expect("git should be installed to run these tests")
}

fn published_vault(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("samong.toml"),
        "[vault]\nname = \"SRE Handbook\"\ndescription = \"Ops knowledge\"\n\
         version = \"1.0.0\"\nlicense = \"CC-BY-4.0\"\n",
    )
    .unwrap();
    fs::write(
        dir.join("Runbook.md"),
        "# Runbook\n\nrestart the frobnicator\n",
    )
    .unwrap();
    git(dir, &["init", "--quiet", "-b", "main"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "--quiet", "-m", "v1.0.0"]);
}

fn buyer_with_installed(buyer: &Path, seller: &Path) {
    fs::write(
        buyer.join("Mine.md"),
        "# Mine\n\nmy own frobnicator notes\n",
    )
    .unwrap();
    samong(buyer)
        .args(["vault", "install"])
        .arg(seller.to_str().unwrap())
        .args(["--name", "handbook"])
        .assert()
        .success();
}

/// The reader has to be told whose notes these are and on what terms, in the
/// results themselves — not in a manifest they would have to know to go and read.
#[test]
fn search_results_say_which_vault_they_came_from_and_under_what_licence() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    let buyer = tempfile::tempdir().unwrap();
    buyer_with_installed(buyer.path(), seller.path());

    samong(buyer.path())
        .args(["search", "frobnicator", "--limit", "10"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("vendor/handbook/Runbook.md")
                .and(predicate::str::contains("from SRE Handbook · CC-BY-4.0")),
        );
}

/// The reader's own notes must stay unmarked. A badge on everything is a badge
/// on nothing.
#[test]
fn the_readers_own_notes_are_not_attributed_to_anyone() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    let buyer = tempfile::tempdir().unwrap();
    buyer_with_installed(buyer.path(), seller.path());

    let output = samong(buyer.path())
        .args(["search", "frobnicator", "--limit", "10"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let own = stdout
        .lines()
        .position(|line| line.starts_with("Mine.md:"))
        .expect("the buyer's own note is a hit too");
    assert!(
        !stdout.lines().nth(own + 1).unwrap_or("").contains("from "),
        "own notes carry no attribution:\n{stdout}"
    );
}

/// A vault that says nothing about its licence is the dangerous case, so silence
/// is reported rather than left out.
#[test]
fn a_vault_with_no_stated_licence_says_so_in_every_result() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    fs::write(
        seller.path().join("samong.toml"),
        "[vault]\nname = \"Notes\"\n",
    )
    .unwrap();
    git(seller.path(), &["add", "-A"]);
    git(seller.path(), &["commit", "--quiet", "-m", "drop licence"]);

    let buyer = tempfile::tempdir().unwrap();
    buyer_with_installed(buyer.path(), seller.path());

    samong(buyer.path())
        .args(["search", "frobnicator", "--limit", "10"])
        .assert()
        .success()
        .stdout(predicate::str::contains("from Notes · licence not stated"));
}

#[test]
fn verify_reports_an_unsigned_vault_as_unproven_without_failing() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    let buyer = tempfile::tempdir().unwrap();
    buyer_with_installed(buyer.path(), seller.path());

    samong(buyer.path())
        .args(["vault", "verify"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("handbook")
                .and(predicate::str::contains("not signed"))
                .and(predicate::str::contains("unproven")),
        );
}

/// `--require-signature` is for the reader who has decided that unproven is not
/// good enough. Same vault, same output, different exit code.
#[test]
fn require_signature_turns_unproven_into_a_failure() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    let buyer = tempfile::tempdir().unwrap();
    buyer_with_installed(buyer.path(), seller.path());

    samong(buyer.path())
        .args(["vault", "verify", "--require-signature"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("could not be proven"));
}

/// A reference note is read-only through Samong, but nothing stops an editor, a
/// script, or a careless `cp` — and an unsigned `.md` dropped into an installed
/// vault would appear in search attributed to the publisher.
#[test]
fn verify_notices_content_that_the_publisher_never_published() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    let buyer = tempfile::tempdir().unwrap();
    buyer_with_installed(buyer.path(), seller.path());

    fs::write(
        buyer.path().join("vendor/handbook/Planted.md"),
        "# Planted\n\nnot theirs\n",
    )
    .unwrap();

    samong(buyer.path())
        .args(["vault", "verify"])
        .assert()
        .failure()
        .stdout(
            predicate::str::contains("no longer matches what was published")
                .and(predicate::str::contains("Planted.md")),
        )
        .stderr(predicate::str::contains("failed"));
}

/// The cheapest attack on signature pinning is to stop signing. A pin without a
/// matching signature has to stop the update *before* the content lands, which
/// is testable without any crypto: pin a key the upstream commits do not carry.
#[test]
fn a_pinned_vault_refuses_an_update_that_is_not_signed_by_the_same_key() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    let buyer = tempfile::tempdir().unwrap();
    buyer_with_installed(buyer.path(), seller.path());

    let installed = buyer.path().join("vendor/handbook");
    git(&installed, &["config", "--local", "samong.signer", "KEY-A"]);
    let before = String::from_utf8(git_raw(&installed, &["rev-parse", "HEAD"]).stdout).unwrap();

    fs::write(seller.path().join("Escalation.md"), "# Escalation\n").unwrap();
    git(seller.path(), &["add", "-A"]);
    git(seller.path(), &["commit", "--quiet", "-m", "v1.1.0"]);

    samong(buyer.path())
        .args(["vault", "update"])
        .assert()
        .success() // one vault failing must not fail the command
        .stdout(
            predicate::str::contains("refusing to update")
                .and(predicate::str::contains("KEY-A"))
                .and(predicate::str::contains("Nothing has been changed on disk")),
        );

    let after = String::from_utf8(git_raw(&installed, &["rev-parse", "HEAD"]).stdout).unwrap();
    assert_eq!(before, after, "the refused commit must not have landed");
    assert!(
        !buyer.path().join("vendor/handbook/Escalation.md").exists(),
        "refused content must not reach the working tree"
    );

    // And verify says the same thing the update did, from a standing start.
    samong(buyer.path())
        .args(["vault", "verify"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("MISMATCH"));
}

/// Dropping the pin is how a reader accepts a genuine key change. It has to
/// actually work, or the refusal above is a dead end rather than a checkpoint.
#[test]
fn dropping_the_pin_lets_a_deliberate_update_through() {
    let seller = tempfile::tempdir().unwrap();
    published_vault(seller.path());
    let buyer = tempfile::tempdir().unwrap();
    buyer_with_installed(buyer.path(), seller.path());

    let installed = buyer.path().join("vendor/handbook");
    git(&installed, &["config", "--local", "samong.signer", "KEY-A"]);
    fs::write(seller.path().join("Escalation.md"), "# Escalation\n").unwrap();
    git(seller.path(), &["add", "-A"]);
    git(seller.path(), &["commit", "--quiet", "-m", "v1.1.0"]);

    git(&installed, &["config", "--unset", "samong.signer"]);
    samong(buyer.path())
        .args(["vault", "update"])
        .assert()
        .success()
        .stdout(predicate::str::contains("handbook: "));
    assert!(buyer.path().join("vendor/handbook/Escalation.md").exists());
}

/// The whole point of pinning is that a *signed* vault stays signed by the same
/// person. That needs a real signature, which needs SSH signing support in git
/// and an `ssh-keygen` — present on every CI runner and on most developer
/// machines, but not something to fail the suite over when it is missing.
#[test]
fn a_signed_vault_pins_its_publisher_and_verifies_against_it() {
    let seller = tempfile::tempdir().unwrap();
    let keys = tempfile::tempdir().unwrap();
    let key = keys.path().join("id");
    let keygen = Proc::new("ssh-keygen")
        .args([
            "-q",
            "-t",
            "ed25519",
            "-N",
            "",
            "-C",
            "pub@example.com",
            "-f",
        ])
        .arg(&key)
        .output();
    match keygen {
        Ok(out) if out.status.success() => {}
        _ => {
            eprintln!("skipped: no usable ssh-keygen here");
            return;
        }
    }
    let public = fs::read_to_string(key.with_extension("pub")).unwrap();
    let allowed = keys.path().join("allowed_signers");
    fs::write(&allowed, format!("pub@example.com {public}")).unwrap();

    published_vault(seller.path());
    let signing: Vec<String> = vec![
        "-c".into(),
        "gpg.format=ssh".into(),
        "-c".into(),
        format!("user.signingkey={}", key.display()).replace('\\', "/"),
        "-c".into(),
        format!("gpg.ssh.allowedSignersFile={}", allowed.display()).replace('\\', "/"),
    ];
    let mut args: Vec<String> = signing.clone();
    args.extend([
        "commit".into(),
        "--quiet".into(),
        "-S".into(),
        "--allow-empty".into(),
        "-m".into(),
        "signed".into(),
    ]);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = git_raw(seller.path(), &refs);
    if !out.status.success() {
        eprintln!(
            "skipped: git here cannot sign with ssh: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }

    let buyer = tempfile::tempdir().unwrap();
    // The buyer needs the publisher's key on file to check anything at all —
    // exactly the step a real buyer takes when the seller publishes their key.
    fs::write(buyer.path().join("Mine.md"), "# Mine\n").unwrap();
    let mut install = samong(buyer.path());
    install
        .args(["vault", "install"])
        .arg(seller.path().to_str().unwrap())
        .args(["--name", "handbook"]);
    apply_signing_env(&mut install, &allowed);
    install
        .assert()
        .success()
        .stdout(predicate::str::contains("pinned that key"));

    let mut verify = samong(buyer.path());
    verify.args(["vault", "verify", "--require-signature"]);
    apply_signing_env(&mut verify, &allowed);
    verify
        .assert()
        .success()
        .stdout(predicate::str::contains("good signature"));
}

/// Point git at the publisher's key without writing to the developer's own
/// `~/.gitconfig`.
fn apply_signing_env(cmd: &mut Command, allowed: &Path) {
    cmd.env("GIT_CONFIG_COUNT", "2")
        .env("GIT_CONFIG_KEY_0", "gpg.format")
        .env("GIT_CONFIG_VALUE_0", "ssh")
        .env("GIT_CONFIG_KEY_1", "gpg.ssh.allowedSignersFile")
        .env(
            "GIT_CONFIG_VALUE_1",
            allowed.display().to_string().replace('\\', "/"),
        );
}
