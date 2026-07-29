//! Phase 13 — reference notes: one brain per project, including knowledge that
//! git does not track.
//!
//! `.gitignore` answers "what do I distribute?". A knowledge base has to answer
//! "what do I learn from?" — and vendored documentation (Next.js ships 400-odd
//! Markdown files inside `node_modules`) sits exactly where those two answers
//! disagree. `scope.include` pulls those in as *reference notes*: same vault,
//! same index, but read-only and machine-local.

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

/// Every invocation gets its own registry, inside the vault's own temp dir.
/// Tests run in parallel and redb takes an exclusive lock, so sharing one
/// registry makes them collide — which is exactly how CI failed while this
/// passed locally. Pointing at the real `~/.config/samong` would also let a test
/// mutate the registry someone actually uses. The dir is a dot-dir, so the scope
/// walker never counts it as notes.
fn samong(cwd: &Path) -> Command {
    let mut cmd = Command::cargo_bin("samong").unwrap();
    cmd.env("SAMONG_CONFIG_DIR", cwd.join(".samong-test-config"))
        .current_dir(cwd);
    cmd
}

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

/// A project whose dependency ships documentation worth learning from.
fn vault_with_vendored_docs() -> tempfile::TempDir {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();

    write(root, ".gitignore", "node_modules/\n");
    write(
        root,
        "samong.toml",
        "[scope]\ninclude = [\"node_modules/next/dist/docs\"]\n",
    );
    write(
        root,
        "PROJECT_OVERVIEW.md",
        "# Overview\n\nwe follow [[installation]] closely\n",
    );

    // The docs we want.
    write(
        root,
        "node_modules/next/dist/docs/01-app/installation.md",
        "# installation\n\nrun npx create-next-app to scaffold the project\n",
    );
    write(
        root,
        "node_modules/next/dist/docs/01-app/routing.md",
        "# routing\n\nfile-system based router using folders\n",
    );
    // Noise in the same dependency tree that we do not want.
    write(
        root,
        "node_modules/next/README.md",
        "# next\n\npackage readme boilerplate\n",
    );
    write(
        root,
        "node_modules/left-pad/CHANGELOG.md",
        "# changelog\n\ndependency release notes\n",
    );
    vault
}

#[test]
fn vendored_docs_join_the_same_vault_without_the_surrounding_noise() {
    let vault = vault_with_vendored_docs();

    samong(vault.path()).arg("list").assert().success().stdout(
        predicate::str::contains("PROJECT_OVERVIEW")
            .and(predicate::str::contains("installation"))
            .and(predicate::str::contains("routing"))
            // Same dependency tree, outside the include root.
            .and(predicate::str::contains("changelog").not()),
    );

    // And they are searchable as part of the project's own brain.
    samong(vault.path())
        .args(["search", "create-next-app"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "node_modules/next/dist/docs/01-app/installation.md",
        ));

    // A project note linking to a reference note resolves — one brain, not two.
    samong(vault.path())
        .args(["links", "installation"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<- PROJECT_OVERVIEW"));
}

#[test]
fn doctor_separates_project_notes_from_reference_notes() {
    let vault = vault_with_vendored_docs();

    samong(vault.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("include: node_modules/next/dist/docs — present").and(
                predicate::str::contains("1 project note(s) + 2 reference note(s)"),
            ),
        );
}

/// Vendored docs collide with themselves constantly — Next's own docs mirror
/// ~100 page names across its two routers — and nothing can be done about it.
/// Reporting all of them buries the collision that actually matters: one that
/// touches a project note, where a `[[link]]` can land somewhere unintended.
#[test]
fn doctor_separates_collisions_that_matter_from_vendored_noise() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    write(root, ".gitignore", "node_modules/\n");
    write(
        root,
        "samong.toml",
        "[scope]\ninclude = [\"node_modules/next/dist/docs\"]\n",
    );
    // A project note sharing a title with a docs page: worth knowing about.
    write(root, "routing.md", "# routing\n\nour own routing notes\n");
    write(
        root,
        "node_modules/next/dist/docs/app/routing.md",
        "# routing\n",
    );
    // Two docs pages sharing a title: pure vendored noise.
    write(
        root,
        "node_modules/next/dist/docs/app/index.md",
        "# index\n",
    );
    write(
        root,
        "node_modules/next/dist/docs/pages/index.md",
        "# index\n",
    );

    samong(root).arg("doctor").assert().success().stdout(
        predicate::str::contains("1 ambiguous title(s) involving project notes")
            .and(predicate::str::contains("routing.md"))
            .and(predicate::str::contains(
                "1 more title(s) collide only among reference notes",
            )),
    );
}

/// The failure this guard exists for: an agent saving what it learned under a
/// title that happens to match a docs page would overwrite a dependency's file,
/// and the next `npm install` would erase it without a trace.
#[test]
fn reference_notes_are_read_only() {
    let vault = vault_with_vendored_docs();
    let doc = vault
        .path()
        .join("node_modules/next/dist/docs/01-app/installation.md");
    let before = fs::read_to_string(&doc).unwrap();

    for (args, verb) in [
        (vec!["delete", "installation"], "delete"),
        (vec!["rename", "installation", "install-guide"], "rename"),
    ] {
        samong(vault.path())
            .args(&args)
            .assert()
            .failure()
            .stderr(predicate::str::contains(verb).and(predicate::str::contains("reference note")));
    }

    assert_eq!(
        fs::read_to_string(&doc).unwrap(),
        before,
        "the dependency's file must be untouched"
    );
    assert!(doc.exists(), "and must still exist");
}

#[test]
fn renaming_a_project_note_leaves_reference_notes_alone() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    write(root, ".gitignore", "node_modules/\n");
    write(
        root,
        "samong.toml",
        "[scope]\ninclude = [\"node_modules/next/dist/docs\"]\n",
    );
    write(root, "Target.md", "# Target\n");
    write(root, "Source.md", "# Source\n\nsee [[Target]]\n");
    // A dependency's docs mentioning the same title must not be rewritten.
    let vendored = "node_modules/next/dist/docs/mentions.md";
    write(
        root,
        vendored,
        "# mentions\n\nrefers to [[Target]] as well\n",
    );

    samong(root)
        .args(["rename", "Target", "Renamed"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("updated 1 link(s) in 1 note(s)").and(
                predicate::str::contains("left 1 read-only reference note(s) untouched"),
            ),
        );

    assert!(fs::read_to_string(root.join("Source.md"))
        .unwrap()
        .contains("[[Renamed]]"));
    assert!(
        fs::read_to_string(root.join(vendored))
            .unwrap()
            .contains("[[Target]]"),
        "the dependency's file keeps its original text"
    );
}

/// `samong.toml` travels with the repo; `node_modules` does not. Every other
/// machine — and any server that only has the git history — will find the
/// include root missing, and that must not be fatal or silent.
#[test]
fn a_missing_include_root_warns_but_never_fails() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    write(
        root,
        "samong.toml",
        "[scope]\ninclude = [\"node_modules/next/dist/docs\"]\n",
    );
    write(root, "Own.md", "# Own\n\nthe project's own note\n");

    // Indexing succeeds, warns, and still finds the project's own notes.
    samong(root).arg("reindex").assert().success().stdout(
        predicate::str::contains("warning: scope.include")
            .and(predicate::str::contains("node_modules/next/dist/docs")),
    );

    samong(root)
        .args(["search", "own note"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Own.md"));

    samong(root)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("NOT on this machine"));
}

/// A link into a reference source that is not installed should not read as rot.
#[test]
fn broken_explains_that_missing_reference_sources_may_resolve_later() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    write(
        root,
        "samong.toml",
        "[scope]\ninclude = [\"node_modules/next/dist/docs\"]\n",
    );
    write(root, "Own.md", "# Own\n\nfollows [[installation]]\n");

    samong(root).arg("broken").assert().success().stdout(
        predicate::str::contains("Own -> [[installation]]").and(predicate::str::contains(
            "may resolve once they are installed",
        )),
    );
}

#[test]
fn doctor_points_at_include_when_dependency_docs_were_skipped() {
    let vault = tempfile::tempdir().unwrap();
    let root = vault.path();
    write(root, ".gitignore", "node_modules/\n");
    write(root, "Own.md", "# own\n");
    write(root, "node_modules/next/dist/docs/a.md", "# a\n");
    write(root, "node_modules/next/dist/docs/b.md", "# b\n");

    // No include configured yet: doctor must name the right lever, since
    // .samongignore cannot reopen a pruned dependency directory.
    samong(root).arg("doctor").assert().success().stdout(
        predicate::str::contains("inside dependency directories")
            .and(predicate::str::contains("scope.include")),
    );
}
