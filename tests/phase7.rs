//! Phase 7: samong-mcp speaks real MCP over stdio — spawn the binary, feed
//! it a full JSON-RPC session, and check every response.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn request(id: u64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

fn tool_call(id: u64, name: &str, arguments: Value) -> String {
    request(
        id,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    )
}

/// The tool result's concatenated text content.
fn tool_text(response: &Value) -> String {
    response["result"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect()
}

#[test]
fn mcp_session_covers_every_tool() {
    // ---- fixture: registry + two vaults with Thai content ----
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    let brain = root.path().join("brain");
    let projects = root.path().join("projects");
    fs::create_dir_all(&brain).unwrap();
    fs::create_dir_all(&projects).unwrap();
    fs::write(
        brain.join("การลงทุน.md"),
        "# การลงทุน\n\nตลาดหลักทรัพย์แห่งประเทศไทยประกาศดัชนีใหม่ ดู [[projects/แผนงาน]]\n",
    )
    .unwrap();
    fs::write(projects.join("แผนงาน.md"), "# แผนงาน\n\nแผนหลักของปีนี้\n").unwrap();

    std::env::set_var("SAMONG_CONFIG_DIR", &config);
    let registry = samong::registry::Registry::open().unwrap();
    let brain_root = registry.add("brain", &brain).unwrap();
    let projects_root = registry.add("projects", &projects).unwrap();
    samong::indexer::reindex(&brain_root, false).unwrap();
    samong::indexer::reindex(&projects_root, false).unwrap();
    drop(registry); // one redb handle per process: release before the child runs

    // ---- drive the server: write the whole session, read all responses ----
    let session = [
        request(1, "initialize", json!({ "protocolVersion": "2025-03-26" })),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string(),
        request(2, "tools/list", json!({})),
        tool_call(3, "list_vaults", json!({})),
        tool_call(4, "search_notes", json!({ "query": "ตลาดหลักทรัพย์" })),
        tool_call(
            5,
            "read_note",
            json!({ "vault": "brain", "title": "การลงทุน" }),
        ),
        tool_call(
            6,
            "save_note",
            json!({
                "vault": "brain",
                "title": "บทเรียน Rust",
                "content": "# บทเรียน Rust\n\nredb เปิดได้ handle เดียวต่อโปรเซส ดู [[การลงทุน]]\n"
            }),
        ),
        tool_call(
            7,
            "get_links",
            json!({ "vault": "brain", "title": "การลงทุน" }),
        ),
        tool_call(
            8,
            "read_note",
            json!({ "vault": "brain", "title": "../evil" }),
        ),
        request(9, "nonsense/method", json!({})),
    ]
    .join("\n");

    let mut child = Command::new(env!("CARGO_BIN_EXE_samong-mcp"))
        .env("SAMONG_CONFIG_DIR", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(session.as_bytes())
        .unwrap(); // dropping stdin closes the pipe -> server exits after replying
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());

    let mut by_id = std::collections::HashMap::new();
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        let v: Value = serde_json::from_str(line).expect("every stdout line is JSON");
        by_id.insert(v["id"].as_u64().unwrap(), v);
    }

    // initialize: handshake succeeded, notification produced no response
    assert_eq!(by_id[&1]["result"]["serverInfo"]["name"], "samong-mcp");
    assert_eq!(by_id.len(), 9);

    // tools/list: all six tools
    assert_eq!(by_id[&2]["result"]["tools"].as_array().unwrap().len(), 6);

    // list_vaults
    let text = tool_text(&by_id[&3]);
    assert!(
        text.contains("brain") && text.contains("projects"),
        "{text}"
    );

    // Thai mid-sentence search across vaults, highlight markers rewritten
    let text = tool_text(&by_id[&4]);
    assert!(text.contains("brain/การลงทุน"), "{text}");
    assert!(text.contains("**ตลาดหลักทรัพย์**"), "{text}");

    // read_note returns raw markdown
    assert!(tool_text(&by_id[&5]).contains("[[projects/แผนงาน]]"));

    // save_note persisted to disk and reindexed
    assert_eq!(by_id[&6]["result"]["isError"], false);
    assert!(brain.join("บทเรียน Rust.md").exists());

    // get_links: forward cross-vault target + backlink from the saved note
    let text = tool_text(&by_id[&7]);
    assert!(text.contains("projects/แผนงาน"), "{text}");
    assert!(text.contains("บทเรียน Rust"), "{text}");

    // path traversal rejected as a tool error, not a crash
    assert_eq!(by_id[&8]["result"]["isError"], true);

    // unknown method -> JSON-RPC error
    assert_eq!(by_id[&9]["error"]["code"], -32601);
}
