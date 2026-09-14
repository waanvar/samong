//! `samong eval` scores search against real questions, and says so honestly.
//!
//! The thing being guarded here is not the arithmetic — that is unit-tested in
//! `eval.rs` — but the two ways a measurement can lie: reporting a search
//! failure when the question set has a typo in it, and reporting a good score
//! for a vault that answers questions it has no answer to.

use std::fs;
use std::process::Command;

fn samong(vault: &std::path::Path, config: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_samong"));
    command.current_dir(vault).env("SAMONG_CONFIG_DIR", config);
    command
}

/// A vault with two notes that answer something and nothing about signing keys.
fn fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let vault = root.path().join("brain");
    let config = root.path().join("config");
    fs::create_dir_all(vault.join("ops")).unwrap();
    fs::write(
        vault.join("ops/tomcat-jdbc.md"),
        "# Tomcat ต่อฐานข้อมูลไม่ได้\n\n\
         อาการคือโหลด JDBC driver ไม่สำเร็จ ต้องวางไฟล์ driver ไว้ใน lib ของ Tomcat\n",
    )
    .unwrap();
    fs::write(
        vault.join("ops/deploy.md"),
        "# Deploying the service\n\nRun the pipeline and watch the health check.\n",
    )
    .unwrap();
    (root, vault, config)
}

#[test]
fn a_typo_in_an_answer_key_is_reported_as_a_broken_question_set() {
    let (_root, vault, config) = fixture();
    let questions = vault.join("questions.toml");
    fs::write(
        &questions,
        "[[question]]\n\
         ask = \"ทำไม tomcat ต่อ db ไม่ได้\"\n\
         answers = [\"ops/tomcat-jbdc.md\"]\n",
    )
    .unwrap();

    let output = samong(&vault, &config)
        .args(["eval", "questions.toml"])
        .output()
        .unwrap();

    // The defect this guards: scoring it as a miss would report the search as
    // broken and send someone to fix ranking that is working fine.
    assert!(
        !output.status.success(),
        "a bad answer key must fail loudly"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ops/tomcat-jbdc.md"), "{stderr}");
    assert!(
        !stderr.contains("hit@"),
        "no score for a broken set: {stderr}"
    );
}

#[test]
fn an_empty_question_set_is_refused_rather_than_scored_as_perfect() {
    let (_root, vault, config) = fixture();
    fs::write(vault.join("questions.toml"), "# to be filled in\n").unwrap();

    let output = samong(&vault, &config)
        .args(["eval", "questions.toml"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no [[question]] entries"), "{stderr}");
}

#[test]
fn a_question_the_vault_cannot_answer_is_counted_apart_from_the_hits() {
    let (_root, vault, config) = fixture();
    fs::write(
        vault.join("questions.toml"),
        "[[question]]\n\
         ask = \"ทำไม tomcat ต่อ db ไม่ได้\"\n\
         answers = [\"ops/tomcat-jdbc.md\"]\n\
         \n\
         [[question]]\n\
         ask = \"zzzqqq unrelated nonsense token\"\n\
         answers = []\n",
    )
    .unwrap();

    let output = samong(&vault, &config)
        .args(["eval", "questions.toml"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Thai mid-sentence search finds the note, and the report says so.
    assert!(
        stdout.contains("2 questions") && stdout.contains("1 the vault should answer"),
        "{stdout}"
    );
    assert!(stdout.contains("hit@1"), "{stdout}");
    assert!(
        stdout.contains("1/1"),
        "the answerable question was found: {stdout}"
    );
    assert!(stdout.contains("MRR"), "{stdout}");

    // And the unanswerable one is reported on its own line, never folded into
    // the hit rate.
    assert!(stdout.contains("answered anyway"), "{stdout}");
    assert!(stdout.contains("1 it should not"), "{stdout}");
}

/// A score with nothing behind it cannot be acted on, so a miss has to name the
/// question, what was wanted, and what came back instead.
#[test]
fn a_miss_prints_what_came_back_instead() {
    let (_root, vault, config) = fixture();
    fs::write(
        vault.join("questions.toml"),
        "[[question]]\n\
         ask = \"deploying the service\"\n\
         answers = [\"ops/tomcat-jdbc.md\"]\n",
    )
    .unwrap();

    let output = samong(&vault, &config)
        .args(["eval", "questions.toml"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("to look at"), "{stdout}");
    assert!(stdout.contains("deploying the service"), "{stdout}");
    assert!(stdout.contains("wanted: ops/tomcat-jdbc.md"), "{stdout}");
    assert!(
        stdout.contains("got:") && stdout.contains("ops/deploy.md"),
        "the miss names what came back instead: {stdout}"
    );
}

/// Every line `samong list` prints has to work as an answer key.
///
/// It did not. `list` printed each note's *title* while `eval` matched on its
/// *key*, so a question set built the way the error message told you to build it
/// — "vault-relative paths, as printed by `samong list`" — was rejected for
/// naming no note in the vault. The two commands disagreed about what a note is
/// called, and the one that was wrong was the one telling you where to look.
///
/// This walks the whole list rather than checking one line, and it does it
/// through the CLI rather than by comparing two functions: the defect lived in
/// the gap between the binary's output and the binary's input, which is a place
/// a unit test cannot stand.
#[test]
fn every_line_of_samong_list_is_a_usable_answer_key() {
    let (_root, vault, config) = fixture();
    // Two notes that share a title. `list` printed "README" twice here, which
    // named neither file — the output was not just wrong for eval, it was
    // ambiguous on its own terms.
    fs::write(vault.join("README.md"), "# README\n\nroot readme\n").unwrap();
    fs::write(vault.join("ops/README.md"), "# README\n\nops readme\n").unwrap();

    let listed = samong(&vault, &config).arg("list").output().unwrap();
    assert!(listed.status.success());
    let keys: Vec<String> = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(keys.len(), 4, "four notes, four lines: {keys:?}");
    assert!(
        keys.contains(&"README.md".to_string()) && keys.contains(&"ops/README.md".to_string()),
        "the duplicate titles are told apart: {keys:?}"
    );

    let questions: String = keys
        .iter()
        .map(|key| format!("[[question]]\nask = \"readme\"\nanswers = [\"{key}\"]\n\n"))
        .collect();
    fs::write(vault.join("questions.toml"), questions).unwrap();

    let output = samong(&vault, &config)
        .args(["eval", "questions.toml"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "eval rejected a key that `samong list` printed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Thai has to survive the report, in both directions.
///
/// The output went through `{:?}`, and Rust's Debug for a string escapes every
/// grapheme-extended char — which is nearly every Thai vowel and tone mark. A
/// question came back as "ปร\u{e31}บเป\u{e47}น" on the one line a reader has to
/// recognise to act on it, and a mistyped answer key the same way. For a search
/// engine whose reason to exist is Thai segmentation, the report is the last
/// place that should be unreadable in Thai.
#[test]
fn thai_survives_the_report_unescaped() {
    let (_root, vault, config) = fixture();

    // A miss: the note that answers it is not what "deploying" retrieves.
    fs::write(
        vault.join("questions.toml"),
        "[[question]]\n\
         ask = \"ปรับเป็นภาษาไทยหรือยัง\"\n\
         answers = [\"ops/tomcat-jdbc.md\"]\n",
    )
    .unwrap();
    let output = samong(&vault, &config)
        .args(["eval", "questions.toml"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ปรับเป็นภาษาไทยหรือยัง"),
        "the question is printed as written: {stdout}"
    );
    assert!(
        !stdout.contains("\\u{"),
        "nothing in the report is escaped: {stdout}"
    );

    // And a broken answer key, where the key itself is Thai as well.
    fs::write(
        vault.join("questions.toml"),
        "[[question]]\n\
         ask = \"ตั้งค่าฐานข้อมูลยังไง\"\n\
         answers = [\"ops/การตั้งค่า.md\"]\n",
    )
    .unwrap();
    let output = samong(&vault, &config)
        .args(["eval", "questions.toml"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "a bad answer key must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ops/การตั้งค่า.md") && stderr.contains("ตั้งค่าฐานข้อมูลยังไง"),
        "both halves stay readable: {stderr}"
    );
    assert!(!stderr.contains("\\u{"), "nothing escaped: {stderr}");
}
