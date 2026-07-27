//! Phase 10 — vault scope, and Phase 11 — path-keyed notes.
//!
//! The bug these cover, as reported from a real project: a vault pointed at a
//! repository root indexed `node_modules` too, so search returned `README`
//! 300-odd times and `CHANGELOG` 150-odd times while the project's three actual
//! notes drowned in it.

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

fn banyan() -> Command {
    Command::cargo_bin("banyan").unwrap()
}

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

/// A vault that looks like a typical JavaScript project: a handful of real
/// notes at the root, and a dependency tree full of Markdown.
fn repo_shaped_vault() -> tempfile::TempDir {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();

    write(root, ".gitignore", "node_modules/\ndist/\n");
    write(
        root,
        "PROJECT_OVERVIEW.md",
        "# Overview\n\nsee [[AGENTS]]\n",
    );
    write(root, "AGENTS.md", "# Agents\n\nhow agents work here\n");
    write(root, "CLAUDE.md", "# Claude\n\nproject instructions\n");

    // Dependencies: the noise.
    for dep in ["left-pad", "chalk", "lodash"] {
        write(
            root,
            &format!("node_modules/{dep}/README.md"),
            "# readme\n\nthis is a dependency readme\n",
        );
        write(
            root,
            &format!("node_modules/{dep}/CHANGELOG.md"),
            "# changelog\n\ndependency release notes\n",
        );
    }
    write(root, "dist/bundle-notes.md", "# build output\n");
    vault
}

#[test]
fn a_repo_root_vault_indexes_only_the_projects_own_notes() {
    let vault = repo_shaped_vault();

    banyan()
        .current_dir(vault.path())
        .arg("list")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("AGENTS")
                .and(predicate::str::contains("CLAUDE"))
                .and(predicate::str::contains("PROJECT_OVERVIEW"))
                // Dependency and build-output notes are not notes.
                .and(predicate::str::contains("CHANGELOG").not())
                .and(predicate::str::contains("bundle-notes").not()),
        );
}

#[test]
fn search_is_not_flooded_by_dependency_readmes() {
    let vault = repo_shaped_vault();

    banyan()
        .current_dir(vault.path())
        .args(["search", "dependency"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no results"));

    banyan()
        .current_dir(vault.path())
        .args(["search", "agents"])
        .assert()
        .success()
        .stdout(predicate::str::contains("AGENTS"));
}

#[test]
fn doctor_reports_scope_and_what_it_skipped() {
    let vault = repo_shaped_vault();

    banyan()
        .current_dir(vault.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("3 note(s) in scope")
                .and(predicate::str::contains("node_modules"))
                .and(predicate::str::contains("gitignore: respected"))
                .and(predicate::str::contains(
                    "no ambiguous titles among project notes",
                )),
        );
}

#[test]
fn banyanignore_can_re_include_notes_the_repo_gitignores() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    // The repo keeps local notes out of git; the vault wants them indexed.
    write(root, ".gitignore", "notes/\n");
    write(root, ".banyanignore", "!notes/\n");
    write(
        root,
        "notes/Local Thinking.md",
        "# local\n\nprivate research\n",
    );

    banyan()
        .current_dir(root)
        .args(["search", "research"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Local Thinking"));
}

#[test]
fn notes_dir_narrows_a_vault_to_one_subtree() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    write(root, "banyan.toml", "[scope]\nnotes_dir = \"docs\"\n");
    write(root, "docs/Guide.md", "# guide\n\nthe real docs\n");
    write(root, "README.md", "# repo readme\n");

    banyan()
        .current_dir(root)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Guide").and(predicate::str::contains("README").not()));
}

#[test]
fn a_config_typo_fails_loudly_instead_of_widening_the_scope() {
    let vault = tempfile::tempdir().unwrap();
    write(vault.path(), "banyan.toml", "[scope]\nexcludes = [\"x\"]\n");
    write(vault.path(), "Note.md", "# note\n");

    banyan()
        .current_dir(vault.path())
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("banyan.toml"));
}

#[test]
fn notes_sharing_a_title_stay_separate_and_are_reported() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    write(root, "README.md", "# README\n\nroot readme text\n");
    write(root, "docs/README.md", "# README\n\ndocs readme text\n");

    // Both files are indexed, and each hit says which file it came from.
    banyan()
        .current_dir(root)
        .args(["search", "readme text"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("README.md:").and(predicate::str::contains("docs/README.md:")),
        );

    // And the ambiguity is named rather than hidden.
    banyan()
        .current_dir(root)
        .arg("doctor")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("1 ambiguous title(s) involving project notes")
                .and(predicate::str::contains("README.md"))
                .and(predicate::str::contains("docs/README.md")),
        );
}

/// Reindexing the same untouched vault twice must be a no-op. With notes keyed
/// by title, duplicates fought over one mtime entry and every run re-indexed
/// them forever.
#[test]
fn a_vault_with_duplicate_titles_settles_after_one_reindex() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    for dir in ["", "docs", "api", "web"] {
        write(
            root,
            format!("{dir}/README.md").trim_start_matches('/'),
            "# README\n",
        );
    }

    banyan().current_dir(root).arg("reindex").assert().success();
    banyan()
        .current_dir(root)
        .arg("reindex")
        .assert()
        .success()
        .stdout(predicate::str::contains("reindexed 0 note(s)"));
}

#[test]
fn rewriting_identical_bytes_does_not_reindex() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    write(root, "Note.md", "# Note\n\nsome content\n");
    banyan().current_dir(root).arg("reindex").assert().success();

    // Same bytes, new mtime — what a git checkout does to a whole tree.
    let content = fs::read_to_string(root.join("Note.md")).unwrap();
    fs::write(root.join("Note.md"), content).unwrap();

    banyan()
        .current_dir(root)
        .arg("reindex")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("reindexed 0 note(s)")
                .and(predicate::str::contains("1 unchanged despite new mtime")),
        );
}

#[test]
fn renaming_works_when_the_linking_note_lives_in_a_subdirectory() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    write(root, "Target.md", "# Target\n");
    write(
        root,
        "deep/area/Source.md",
        "# Source\n\nlinks [[Target]]\n",
    );

    banyan()
        .current_dir(root)
        .args(["rename", "Target", "Renamed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("updated 1 link(s) in 1 note(s)"));

    let source = fs::read_to_string(root.join("deep/area/Source.md")).unwrap();
    assert!(
        source.contains("[[Renamed]]"),
        "link not rewritten: {source}"
    );
}
