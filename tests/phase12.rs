//! Phase 12 — token budget for AI agents.
//!
//! `search_notes` returns up to `limit` hits *per vault*, so an agent searching
//! every registered vault could get `limit × vaults` results back — context it
//! then pays for on every later turn. The limit means a total, and the default
//! is small enough for an agent that wants one answer rather than a list.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn tool_call(id: u64, name: &str, arguments: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
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

/// Run a session against a fresh `banyan-mcp` and return responses by id.
fn run_session(
    config: &std::path::Path,
    lines: &[String],
) -> std::collections::HashMap<u64, Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_banyan-mcp"))
        .env("BANYAN_CONFIG_DIR", config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(lines.join("\n").as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());

    let mut by_id = std::collections::HashMap::new();
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        let v: Value = serde_json::from_str(line).expect("every stdout line is JSON");
        by_id.insert(v["id"].as_u64().unwrap(), v);
    }
    by_id
}

#[test]
fn search_limit_is_a_total_across_vaults_not_per_vault() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");

    // Three vaults, 12 matching notes each: 36 possible hits.
    std::env::set_var("BANYAN_CONFIG_DIR", &config);
    let registry = banyan::registry::Registry::open().unwrap();
    for name in ["alpha", "beta", "gamma"] {
        let vault = root.path().join(name);
        fs::create_dir_all(&vault).unwrap();
        for i in 0..12 {
            fs::write(
                vault.join(format!("{name} note {i}.md")),
                format!("# {name} {i}\n\nเอกสารนี้พูดถึงตลาดหลักทรัพย์แห่งประเทศไทย\n"),
            )
            .unwrap();
        }
        let vault_root = registry.add(name, &vault).unwrap();
        banyan::indexer::reindex(&vault_root, false).unwrap();
    }
    drop(registry); // one redb handle per process: release before the child runs

    let by_id = run_session(
        &config,
        &[
            tool_call(1, "search_notes", json!({ "query": "ตลาดหลักทรัพย์" })),
            tool_call(
                2,
                "search_notes",
                json!({ "query": "ตลาดหลักทรัพย์", "limit": 3 }),
            ),
            tool_call(
                3,
                "search_notes",
                json!({ "query": "ตลาดหลักทรัพย์", "vault": "alpha", "limit": 2 }),
            ),
        ],
    );

    // Default: the small agent-facing default, not 20 per vault.
    let default_hits = tool_text(&by_id[&1]).lines().count();
    assert_eq!(
        default_hits, 8,
        "default must be the agent budget, got {default_hits} lines"
    );

    // An explicit limit is a total, even though three vaults each had matches.
    let text = tool_text(&by_id[&2]);
    assert_eq!(
        text.lines().count(),
        3,
        "limit must cap the combined result: {text}"
    );

    // Scoped to one vault, the limit still holds.
    assert_eq!(tool_text(&by_id[&3]).lines().count(), 2);
}

#[test]
fn search_limit_is_clamped_and_survives_junk() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    let vault = root.path().join("solo");
    fs::create_dir_all(&vault).unwrap();
    for i in 0..4 {
        fs::write(
            vault.join(format!("note {i}.md")),
            "# note\n\nshared keyword here\n",
        )
        .unwrap();
    }

    std::env::set_var("BANYAN_CONFIG_DIR", &config);
    let registry = banyan::registry::Registry::open().unwrap();
    let vault_root = registry.add("solo", &vault).unwrap();
    banyan::indexer::reindex(&vault_root, false).unwrap();
    drop(registry);

    let by_id = run_session(
        &config,
        &[
            // Zero results is never what a caller wants: clamped up to one.
            tool_call(1, "search_notes", json!({ "query": "keyword", "limit": 0 })),
            // Absurdly large is clamped down, not an error.
            tool_call(
                2,
                "search_notes",
                json!({ "query": "keyword", "limit": 100000 }),
            ),
            // A non-numeric limit falls back to the default instead of failing.
            tool_call(
                3,
                "search_notes",
                json!({ "query": "keyword", "limit": "lots" }),
            ),
        ],
    );

    assert_eq!(tool_text(&by_id[&1]).lines().count(), 1);
    assert_eq!(
        tool_text(&by_id[&2]).lines().count(),
        4,
        "only 4 notes exist"
    );
    assert_eq!(by_id[&3]["result"]["isError"], false);
    assert_eq!(tool_text(&by_id[&3]).lines().count(), 4);
}

#[test]
fn the_limit_is_documented_in_the_tool_schema() {
    // An agent can only use the parameter if the schema tells it the parameter
    // exists and that it is a total.
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    fs::create_dir_all(&config).unwrap();

    let by_id = run_session(
        &config,
        &[json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }).to_string()],
    );
    let tools = by_id[&1]["result"]["tools"].as_array().unwrap();
    let search = tools
        .iter()
        .find(|t| t["name"] == "search_notes")
        .expect("search_notes is exposed");
    let limit = &search["inputSchema"]["properties"]["limit"];
    assert_eq!(limit["type"], "integer");
    let description = limit["description"].as_str().unwrap();
    assert!(
        description.contains("total"),
        "the schema must say the limit is a total: {description}"
    );
}
