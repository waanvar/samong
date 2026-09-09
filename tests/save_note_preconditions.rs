//! `save_note` cannot overwrite a note the agent has not read.
//!
//! The hole this closes: the MCP server has no delete tool on purpose, but
//! `save_note` writes the whole file, so an agent that never read a note could
//! replace all of it with one paragraph — and be told "saved". That is deletion
//! with a friendlier name, and the report of success is the part that makes it
//! dangerous. Every assertion below therefore checks the bytes on disk, not just
//! the error text: a refusal that still wrote is the failure worth catching.

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

/// Run one MCP session against `vault`, returning responses by request id.
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

#[test]
fn an_unread_note_cannot_be_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    let vault = root.path().join("brain");
    fs::create_dir_all(&vault).unwrap();

    let note = vault.join("คู่มือ.md");
    let original = "# คู่มือ\n\nขั้นตอนที่สะสมมาสามปี ห้ามหาย\n";
    fs::write(&note, original).unwrap();

    std::env::set_var("SAMONG_CONFIG_DIR", &config);
    let registry = samong::registry::Registry::open().unwrap();
    let vault_root = registry.add("brain", &vault).unwrap();
    samong::indexer::reindex(&vault_root, false).unwrap();
    drop(registry); // one redb handle per process: release before the child runs

    // ---- pass 1: every way of getting it wrong ----
    let by_id = run(
        &config,
        vec![
            // 1. blind overwrite — the defect itself
            tool_call(
                1,
                "save_note",
                json!({ "vault": "brain", "path": "คู่มือ.md", "content": "# คู่มือ\n\nสั้นนิดเดียว\n" }),
            ),
            // 2. a hash, but not this note's version
            tool_call(
                2,
                "save_note",
                json!({
                    "vault": "brain", "path": "คู่มือ.md",
                    "content": "# คู่มือ\n\nสั้นนิดเดียว\n",
                    "base_hash": "0".repeat(64)
                }),
            ),
            // 3. a hash for a note that is not there: deleted under the agent
            tool_call(
                3,
                "save_note",
                json!({
                    "vault": "brain", "path": "ไม่มีอยู่.md",
                    "content": "# ไม่มีอยู่\n", "base_hash": "0".repeat(64)
                }),
            ),
            // 4. read it, so the next pass has something true to say
            tool_call(
                4,
                "read_note",
                json!({ "vault": "brain", "path": "คู่มือ.md" }),
            ),
        ],
    );

    assert!(is_error(&by_id[&1]), "a blind overwrite must be refused");
    assert!(
        tool_text(&by_id[&1]).contains("read_note"),
        "the refusal has to say what to do instead: {}",
        tool_text(&by_id[&1])
    );
    assert!(is_error(&by_id[&2]), "a stale base_hash must be refused");
    assert!(
        is_error(&by_id[&3]),
        "a base_hash for a missing note must be refused"
    );
    assert!(
        !vault.join("ไม่มีอยู่.md").exists(),
        "a refused save must not create the note"
    );

    // The point of the whole exercise: three refusals, file untouched.
    assert_eq!(
        fs::read_to_string(&note).unwrap(),
        original,
        "a refused save wrote to the note anyway"
    );

    // read_note hands back the hash, above the note and not inside it.
    let read = tool_text(&by_id[&4]);
    let (header, body) = read.split_once("\n\n").unwrap();
    assert!(header.starts_with("[samong base_hash="), "{header}");
    assert_eq!(body, original, "the note itself must come back unaltered");
    let hash = header
        .trim_start_matches("[samong base_hash=")
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();

    // ---- pass 2: the hash read_note gave is the one save_note takes ----
    let updated = "# คู่มือ\n\nขั้นตอนที่สะสมมาสามปี ห้ามหาย\n\nเพิ่มบรรทัดนี้\n";
    let by_id = run(
        &config,
        vec![
            // 5. the header echoed back into the content, hash and all
            tool_call(
                5,
                "save_note",
                json!({
                    "vault": "brain", "path": "คู่มือ.md",
                    "content": format!("[samong base_hash={hash} — not part of the note]\n\n{updated}"),
                    "base_hash": &hash
                }),
            ),
            // 6. the honest edit
            tool_call(
                6,
                "save_note",
                json!({ "vault": "brain", "path": "คู่มือ.md", "content": updated, "base_hash": &hash }),
            ),
            // 7. a brand new note still needs no hash
            tool_call(
                7,
                "save_note",
                json!({ "vault": "brain", "path": "ใหม่.md", "content": "# ใหม่\n\nยังไม่เคยมี\n" }),
            ),
        ],
    );

    assert!(
        is_error(&by_id[&5]),
        "the echoed read_note header must be refused, not saved into the note"
    );
    assert!(!is_error(&by_id[&6]), "{}", tool_text(&by_id[&6]));
    assert_eq!(fs::read_to_string(&note).unwrap(), updated);
    assert!(!is_error(&by_id[&7]), "{}", tool_text(&by_id[&7]));
    assert!(vault.join("ใหม่.md").exists());

    // Nothing dotted or .tmp left in the vault after five writes and refusals.
    let strays: Vec<String> = fs::read_dir(&vault)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(
        strays.is_empty(),
        "temp files left in the vault: {strays:?}"
    );
}
