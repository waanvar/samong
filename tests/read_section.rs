//! Reading one section of a note instead of all of it.
//!
//! The saving is the obvious half. The dangerous half is `base_hash`: `save_note`
//! replaces the whole file, so an agent that read one section and was handed a
//! hash could send that section back as the note's new content and delete the
//! rest, having been told it was editing safely. A partial read must grant no
//! right to write.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn tool_call(id: u64, name: &str, arguments: Value) -> String {
    json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
    .to_string()
}

fn tool_text(response: &Value) -> String {
    response["result"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect()
}

fn is_error(response: &Value) -> bool {
    response["result"]["isError"].as_bool().unwrap()
}

fn run(config: &std::path::Path, session: Vec<String>) -> std::collections::HashMap<u64, Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_samong-mcp"))
        .env("SAMONG_CONFIG_DIR", config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(session.join("\n").as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| {
            let v: Value = serde_json::from_str(line).unwrap();
            (v["id"].as_u64().unwrap(), v)
        })
        .collect()
}

/// Register and index the vault by running the CLI, not by calling the library.
///
/// `Registry::open` reads `SAMONG_CONFIG_DIR` from the environment, and the
/// environment belongs to the whole test binary: two tests setting it race, and
/// the loser registers its vault into the winner's config. Handing the variable
/// to a child process instead keeps each test's fixture its own — which is also
/// what every other integration test here does.
fn register(vault: &std::path::Path, config: &std::path::Path) {
    for args in [vec!["vault", "add", "brain", "."], vec!["reindex"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_samong"))
            .current_dir(vault)
            .env("SAMONG_CONFIG_DIR", config)
            .args(&args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

const RUNBOOK: &str = "\
# Tomcat ต่อฐานข้อมูลไม่ได้

## อาการ

โหลด JDBC driver ไม่สำเร็จ

## วิธีแก้

วาง ifxjdbc.jar ไว้ใน lib ของ Tomcat

## ผลทดสอบ

ผ่านบน Tomcat 8 / Java 8
";

#[test]
fn a_section_read_is_smaller_and_grants_no_right_to_write() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    let vault = root.path().join("brain");
    fs::create_dir_all(&vault).unwrap();
    fs::write(vault.join("tomcat.md"), RUNBOOK).unwrap();

    register(&vault, &config);

    let by_id = run(
        &config,
        vec![
            tool_call(
                1,
                "read_note",
                json!({ "vault": "brain", "path": "tomcat.md" }),
            ),
            tool_call(
                2,
                "read_note",
                json!({ "vault": "brain", "path": "tomcat.md", "section": "วิธีแก้" }),
            ),
            tool_call(
                3,
                "read_note",
                json!({ "vault": "brain", "path": "tomcat.md", "section": "ไม่มีหัวข้อนี้" }),
            ),
            // Case and stray #s should not stand between an agent and its answer.
            tool_call(
                4,
                "read_note",
                json!({ "vault": "brain", "path": "tomcat.md", "section": "## ผลทดสอบ" }),
            ),
        ],
    );

    let whole = tool_text(&by_id[&1]);
    let section = tool_text(&by_id[&2]);

    // The saving, stated as the property that matters rather than a ratio.
    assert!(section.contains("วาง ifxjdbc.jar"), "{section}");
    assert!(
        !section.contains("โหลด JDBC driver"),
        "other sections leaked in"
    );
    assert!(
        !section.contains("ผ่านบน Tomcat 8"),
        "other sections leaked in"
    );
    assert!(
        section.len() < whole.len() / 2,
        "section was not much smaller"
    );

    // The sharp edge: a whole read grants a base_hash, a section read must not.
    assert!(whole.contains("[samong base_hash="), "{whole}");
    assert!(
        !section.contains("base_hash="),
        "a partial read handed out a hash that would authorise replacing the whole note: {section}"
    );
    assert!(
        section.contains("read the whole note"),
        "the section read must say how to get a hash: {section}"
    );

    // A wrong name is an error that names the real headings, so the agent can
    // retry without reading the whole file to find out.
    assert!(is_error(&by_id[&3]));
    let error = tool_text(&by_id[&3]);
    assert!(error.contains("อาการ") && error.contains("วิธีแก้"), "{error}");

    assert!(!is_error(&by_id[&4]), "{}", tool_text(&by_id[&4]));
    assert!(tool_text(&by_id[&4]).contains("ผ่านบน Tomcat 8"));
}

#[test]
fn a_note_without_headings_says_so_instead_of_returning_nothing() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    let vault = root.path().join("brain");
    fs::create_dir_all(&vault).unwrap();
    fs::write(
        vault.join("plain.md"),
        "just a paragraph, no headings at all\n",
    )
    .unwrap();

    register(&vault, &config);

    let by_id = run(
        &config,
        vec![tool_call(
            1,
            "read_note",
            json!({ "vault": "brain", "path": "plain.md", "section": "anything" }),
        )],
    );
    assert!(is_error(&by_id[&1]));
    let error = tool_text(&by_id[&1]);
    assert!(error.contains("no headings"), "{error}");
    assert!(error.contains("without the section argument"), "{error}");
}
