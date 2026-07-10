use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

/// Two registered vaults ("work" and "ideas") plus an isolated registry.
/// work/Source links to [[ideas/Target]] cross-vault and [[Local]] in-vault.
struct TwoVaults {
    _root: tempfile::TempDir,
    config: PathBuf,
    work: PathBuf,
    ideas: PathBuf,
}

fn banyan(config: &Path, cwd: &Path) -> Command {
    let mut cmd = Command::cargo_bin("banyan").expect("binary should build");
    cmd.env("BANYAN_CONFIG_DIR", config).current_dir(cwd);
    cmd
}

fn setup() -> TwoVaults {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    let work = root.path().join("work");
    let ideas = root.path().join("ideas");
    fs::create_dir_all(&work).unwrap();
    fs::create_dir_all(&ideas).unwrap();

    fs::write(
        work.join("Source.md"),
        "# Source\n\ncross link [[ideas/Target]] and local [[Local]]\n",
    )
    .unwrap();
    fs::write(work.join("Local.md"), "# Local\n\nnothing special\n").unwrap();
    fs::write(
        ideas.join("Target.md"),
        "# Target\n\nthe linked-to note with unicorn content\n",
    )
    .unwrap();

    let fixture = TwoVaults {
        config,
        work,
        ideas,
        _root: root,
    };
    for (name, path) in [("work", &fixture.work), ("ideas", &fixture.ideas)] {
        banyan(&fixture.config, path)
            .args(["vault", "add", name])
            .arg(path)
            .assert()
            .success()
            .stdout(predicate::str::contains(format!("registered \"{name}\"")));
    }
    fixture
}

/// Acceptance: two vaults linking across each other — cross-vault backlinks
/// must be reported correctly.
#[test]
fn cross_vault_backlinks_are_visible_from_the_target_vault() {
    let v = setup();

    // From inside "ideas", Target must see the backlink from work/Source.
    banyan(&v.config, &v.ideas)
        .args(["links", "Target", "--all-vaults"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("cross-vault backlinks (1):")
                .and(predicate::str::contains("<- work/Source")),
        );

    // Without the flag, only local links are shown.
    banyan(&v.config, &v.ideas)
        .args(["links", "Target"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("backlinks (0)")
                .and(predicate::str::contains("work/Source").not()),
        );

    // In-vault linking still works untouched (Obsidian compat).
    banyan(&v.config, &v.work)
        .args(["links", "Local"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<- Source"));
}

#[test]
fn vault_list_and_remove_manage_the_registry() {
    let v = setup();

    banyan(&v.config, &v.work)
        .args(["vault", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("work").and(predicate::str::contains("ideas")));

    banyan(&v.config, &v.work)
        .args(["vault", "remove", "ideas"])
        .assert()
        .success();
    banyan(&v.config, &v.work)
        .args(["vault", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ideas").not());

    // Removing twice fails cleanly; duplicate add is rejected.
    banyan(&v.config, &v.work)
        .args(["vault", "remove", "ideas"])
        .assert()
        .failure();
    banyan(&v.config, &v.work)
        .args(["vault", "add", "work"])
        .arg(&v.work)
        .assert()
        .failure();
}

#[test]
fn graph_all_vaults_qualifies_every_node() {
    let v = setup();

    banyan(&v.config, &v.work)
        .args(["graph", "--all-vaults"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("work/Source -> ideas/Target")
                .and(predicate::str::contains("work/Source -> work/Local")),
        );
}

#[test]
fn search_targets_a_specific_vault_or_all() {
    let v = setup();

    // By name, from anywhere (here: inside "work").
    banyan(&v.config, &v.work)
        .args(["search", "--vault", "ideas", "unicorn"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Target"));

    // Across every vault, hits are qualified with the vault name.
    banyan(&v.config, &v.work)
        .args(["search", "--all-vaults", "unicorn"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ideas/Target"));

    banyan(&v.config, &v.work)
        .args(["search", "--vault", "nope", "unicorn"])
        .assert()
        .failure();
}

#[test]
fn broken_understands_cross_vault_links() {
    let v = setup();

    // [[ideas/Target]] resolves in the other vault: not broken.
    banyan(&v.config, &v.work)
        .arg("broken")
        .assert()
        .success()
        .stdout(predicate::str::contains("no broken links"));

    // Delete the target note; now the cross-vault link is genuinely broken.
    fs::remove_file(v.ideas.join("Target.md")).unwrap();
    banyan(&v.config, &v.work)
        .arg("broken")
        .assert()
        .success()
        .stdout(predicate::str::contains("Source -> [[ideas/Target]]"));
}
