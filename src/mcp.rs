//! MCP (Model Context Protocol) server over stdio, so AI agents like
//! Claude Code can use banyan vaults as their knowledge base.
//!
//! MCP is JSON-RPC 2.0, one message per line on stdin/stdout. We implement
//! the minimal method set a tools-only server needs (initialize, tools/list,
//! tools/call, ping) directly — no SDK, no async: requests are handled one
//! at a time, which also keeps redb's one-handle-per-process rule trivially
//! satisfied.
//!
//! Deliberately no delete tool: an agent's brain should accumulate knowledge,
//! not be able to erase it. Deletion stays a human action (CLI / Web UI).

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::graph::Graph;
use crate::indexer;
use crate::ops;
use crate::registry::Registry;
use crate::search;
use crate::vault;

/// Handle one raw JSON-RPC line. Returns the response line, or None for
/// notifications and unparseable input (per JSON-RPC, notifications get no
/// response; garbage on a line-oriented pipe is best skipped).
pub fn handle_line(line: &str) -> Option<String> {
    let message: Value = serde_json::from_str(line).ok()?;
    let id = message.get("id").filter(|v| !v.is_null()).cloned()?;
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);

    let body = match method {
        "initialize" => Ok(initialize_result(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => Ok(call_tool(&params)),
        other => Err(format!("method not found: {other}")),
    };

    Some(
        match body {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(message) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": message }
            }),
        }
        .to_string(),
    )
}

fn initialize_result(params: &Value) -> Value {
    // Echo the client's protocol version; the lenient choice keeps us
    // compatible across spec revisions of this small method set.
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("2024-11-05");
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "banyan-mcp", "version": env!("CARGO_PKG_VERSION") }
    })
}

fn tool_definitions() -> Value {
    let vault_arg = json!({ "type": "string", "description": "Registered vault name" });
    let title_arg = json!({ "type": "string", "description": "Note title (filename without .md)" });
    json!([
        {
            "name": "list_vaults",
            "description": "List every registered knowledge vault (name and path).",
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        },
        {
            "name": "list_notes",
            "description": "List all note titles in a vault.",
            "inputSchema": {
                "type": "object",
                "properties": { "vault": vault_arg },
                "required": ["vault"]
            }
        },
        {
            "name": "read_note",
            "description": "Read the full markdown content of a note.",
            "inputSchema": {
                "type": "object",
                "properties": { "vault": vault_arg, "title": title_arg },
                "required": ["vault", "title"]
            }
        },
        {
            "name": "save_note",
            "description": "Create or overwrite a note with markdown content. Use [[Note Title]] wikilinks to connect knowledge; use [[vault-name/Note Title]] to link across vaults.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault": vault_arg,
                    "title": title_arg,
                    "content": { "type": "string", "description": "Full markdown content" }
                },
                "required": ["vault", "title", "content"]
            }
        },
        {
            "name": "search_notes",
            "description": "Full-text search across notes. Handles Thai text with dictionary word segmentation (finds words mid-sentence). Omit vault to search every vault.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query (Thai or English)" },
                    "vault": vault_arg
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_links",
            "description": "Show a note's connections: forward links, backlinks, and backlinks from other vaults.",
            "inputSchema": {
                "type": "object",
                "properties": { "vault": vault_arg, "title": title_arg },
                "required": ["vault", "title"]
            }
        }
    ])
}

fn call_tool(params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let outcome = match name {
        "list_vaults" => tool_list_vaults(),
        "list_notes" => tool_list_notes(&args),
        "read_note" => tool_read_note(&args),
        "save_note" => tool_save_note(&args),
        "search_notes" => tool_search_notes(&args),
        "get_links" => tool_get_links(&args),
        other => Err(anyhow::anyhow!("unknown tool \"{other}\"")),
    };
    match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Err(err) => json!({
            "content": [{ "type": "text", "text": format!("{err:#}") }],
            "isError": true
        }),
    }
}

// ---------- tool implementations ----------

fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .with_context(|| format!("missing required argument \"{key}\""))
}

fn checked_title(args: &Value) -> Result<&str> {
    let title = str_arg(args, "title")?;
    ops::validate_title(title).map_err(|m| anyhow::anyhow!(m))?;
    Ok(title)
}

fn resolve_vault(registry: &Registry, name: &str) -> Result<PathBuf> {
    registry
        .get(name)?
        .with_context(|| format!("vault \"{name}\" is not registered"))
}

fn tool_list_vaults() -> Result<String> {
    let vaults = Registry::open()?.list()?;
    if vaults.is_empty() {
        return Ok("no vaults registered — run `banyan vault add <name> <path>` first".into());
    }
    Ok(vaults
        .into_iter()
        .map(|(name, path)| format!("{name}\t{}", path.display()))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn tool_list_notes(args: &Value) -> Result<String> {
    let root = resolve_vault(&Registry::open()?, str_arg(args, "vault")?)?;
    let titles: Vec<String> = vault::list_notes(&root)?
        .into_iter()
        .map(|n| n.title)
        .collect();
    if titles.is_empty() {
        return Ok("(vault is empty)".into());
    }
    Ok(titles.join("\n"))
}

fn tool_read_note(args: &Value) -> Result<String> {
    let title = checked_title(args)?;
    let root = resolve_vault(&Registry::open()?, str_arg(args, "vault")?)?;
    let note = vault::find_note(&root, title)?
        .with_context(|| format!("note \"{title}\" does not exist"))?;
    fs::read_to_string(&note.path).with_context(|| format!("reading {}", note.path.display()))
}

fn tool_save_note(args: &Value) -> Result<String> {
    let vault_name = str_arg(args, "vault")?;
    let title = checked_title(args)?;
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .context("missing required argument \"content\"")?;

    let root = resolve_vault(&Registry::open()?, vault_name)?;
    // Update in place when the note lives in a subfolder; create at the root
    // otherwise (same rule as the HTTP API).
    let path = match vault::find_note(&root, title)? {
        Some(existing) => existing.path,
        None => vault::note_path(&root, title),
    };
    fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    let report = indexer::reindex(&root, false)?;
    Ok(format!(
        "saved \"{title}\" in vault \"{vault_name}\" ({report})"
    ))
}

fn tool_search_notes(args: &Value) -> Result<String> {
    let query = str_arg(args, "query")?;
    let registry = Registry::open()?;
    let targets: Vec<(String, PathBuf)> = match args.get("vault").and_then(Value::as_str) {
        Some(name) if !name.is_empty() => vec![(name.to_string(), resolve_vault(&registry, name)?)],
        _ => registry.list()?,
    };

    let mut lines = Vec::new();
    for (name, root) in targets {
        indexer::reindex(&root, false)?;
        for hit in search::query(&root, query)? {
            let snippet = hit
                .snippet
                .replace("<b>", "**")
                .replace("</b>", "**")
                .replace('\n', " ");
            lines.push(format!("{name}/{}: {snippet}", hit.title));
        }
    }
    if lines.is_empty() {
        return Ok("no results".into());
    }
    Ok(lines.join("\n"))
}

fn tool_get_links(args: &Value) -> Result<String> {
    let vault_name = str_arg(args, "vault")?;
    let title = checked_title(args)?;
    let registry = Registry::open()?;
    let root = resolve_vault(&registry, vault_name)?;
    indexer::reindex(&root, false)?;

    let (forward, backlinks) = {
        let graph = Graph::open(&root)?;
        (graph.forward_links(title)?, graph.backlinks(title)?)
    };
    let cross = ops::cross_vault_backlinks(&registry, vault_name, title)?;

    let section = |label: &str, items: &[String]| {
        if items.is_empty() {
            format!("{label}: (none)")
        } else {
            format!(
                "{label}:\n{}",
                items
                    .iter()
                    .map(|i| format!("- {i}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    };
    Ok([
        section("forward links", &forward),
        section("backlinks", &backlinks),
        section("cross-vault backlinks", &cross),
    ]
    .join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, params: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string()
    }

    #[test]
    fn initialize_reports_tools_capability() {
        let response = handle_line(&request(
            "initialize",
            json!({ "protocolVersion": "2025-03-26" }),
        ))
        .unwrap();
        let v: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(v["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(v["result"]["serverInfo"]["name"], "banyan-mcp");
        assert!(v["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn tools_list_exposes_six_tools_and_no_delete() {
        let response = handle_line(&request("tools/list", json!({}))).unwrap();
        let v: Value = serde_json::from_str(&response).unwrap();
        let names: Vec<&str> = v["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"search_notes"));
        assert!(!names.iter().any(|n| n.contains("delete")));
    }

    #[test]
    fn notifications_and_garbage_get_no_response() {
        let notification =
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string();
        assert!(handle_line(&notification).is_none());
        assert!(handle_line("not json at all").is_none());
    }

    #[test]
    fn unknown_method_returns_error() {
        let response = handle_line(&request("bogus/method", json!({}))).unwrap();
        let v: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(v["error"]["code"], -32601);
    }

    #[test]
    fn unknown_tool_is_a_tool_error_not_a_protocol_error() {
        let response = handle_line(&request(
            "tools/call",
            json!({ "name": "erase_everything", "arguments": {} }),
        ))
        .unwrap();
        let v: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(v["result"]["isError"], true);
    }
}
