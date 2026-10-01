//! `--explain` shows what a ranking was built from, and must never disagree
//! with the ranking itself.
//!
//! The scores used to be computed and thrown away: the first similarity floor
//! ever swept on a real vault was swept in a range below every cosine the
//! model produces, and every row came back identical because nothing printed a
//! score. These tests hold the explanation to two promises — it describes the
//! same order plain search returns, and it says why meaning did or did not take
//! part, so "lexical only" is never mistaken for "fused and tuned".

use std::fs;
use std::process::Command;

fn samong(vault: &std::path::Path, config: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_samong"));
    command.current_dir(vault).env("SAMONG_CONFIG_DIR", config);
    command
}

fn fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let vault = root.path().join("brain");
    let config = root.path().join("config");
    fs::create_dir_all(vault.join("ops")).unwrap();
    fs::write(
        vault.join("ops/deploy.md"),
        "# Deploy\n\nRun the deploy pipeline, then watch the deploy health check.\n",
    )
    .unwrap();
    fs::write(
        vault.join("ops/rollback.md"),
        "# Rollback\n\nIf a deploy goes wrong, roll back to the previous tag.\n",
    )
    .unwrap();
    fs::write(
        vault.join("notes.md"),
        "# Notes\n\nNothing about shipping.\n",
    )
    .unwrap();
    (root, vault, config)
}

fn keys(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|line| !line.starts_with(' ') && !line.starts_with("meaning:"))
        .filter_map(|line| line.split_once(".md:").map(|(key, _)| format!("{key}.md")))
        .collect()
}

/// An explanation assembled separately from the ranking is one that can drift
/// from it. Both come from one code path; this holds them to it.
#[test]
fn the_explanation_lists_exactly_what_plain_search_returns() {
    let (_root, vault, config) = fixture();
    let plain = samong(&vault, &config)
        .args(["search", "deploy"])
        .output()
        .unwrap();
    let explained = samong(&vault, &config)
        .args(["search", "deploy", "--explain"])
        .output()
        .unwrap();
    assert!(plain.status.success() && explained.status.success());

    let plain = keys(&String::from_utf8_lossy(&plain.stdout));
    let stdout = String::from_utf8_lossy(&explained.stdout).to_string();
    assert_eq!(keys(&stdout), plain, "same hits, same order: {stdout}");
    assert_eq!(plain.len(), 2, "both deploy notes, not the unrelated one");

    // Every hit gets its own line of evidence, and the words ranking is the
    // order itself when meaning is not taking part.
    assert!(stdout.contains("  words #1"), "{stdout}");
    assert!(stdout.contains("  words #2"), "{stdout}");
}

/// Three situations rank identically — no feature, no embeddings, meaning used
/// — and need three different responses. The explanation has to say which.
#[test]
fn the_explanation_says_why_meaning_did_or_did_not_take_part() {
    let (_root, vault, config) = fixture();
    let output = samong(&vault, &config)
        .args(["search", "deploy", "--explain"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = if cfg!(feature = "semantic") {
        // The fixture vault is never embedded: CI does not download the model.
        "meaning: this vault has no embeddings"
    } else {
        "meaning: not in this build"
    };
    assert!(stdout.contains(expected), "{stdout}");
}

/// `eval --explain` with nothing to measure says so rather than printing an
/// empty table that reads like every cosine was zero.
#[test]
fn eval_explain_without_cosines_says_there_are_none_to_show() {
    let (root, vault, config) = fixture();
    let questions = root.path().join("questions.toml");
    fs::write(
        &questions,
        "[[question]]\nask = \"deploy health check\"\nanswers = [\"ops/deploy.md\"]\n\n\
         [[question]]\nask = \"kubernetes autoscaling\"\nanswers = []\n",
    )
    .unwrap();
    let output = samong(&vault, &config)
        .arg("eval")
        .arg(&questions)
        .arg("--explain")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("hit@1"),
        "the ordinary report still prints: {stdout}"
    );
    assert!(
        stdout.contains("there are no cosines to show"),
        "the absence is named: {stdout}"
    );
    assert!(
        !stdout.contains("answer  closest"),
        "no empty table: {stdout}"
    );
}
