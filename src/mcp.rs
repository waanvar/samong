//! MCP (Model Context Protocol) server over stdio, so AI agents like
//! Claude Code can use samong vaults as their knowledge base.
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

/// Results `search_notes` returns when the caller does not say otherwise.
///
/// Lower than [`search::DEFAULT_LIMIT`] on purpose: an agent pays for every hit
/// as context on every subsequent turn, and usually wants the one note that
/// answers its question rather than a browsable list. It can still ask for more.
const DEFAULT_LIMIT: usize = 8;

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
        "serverInfo": { "name": "samong-mcp", "version": env!("CARGO_PKG_VERSION") }
    })
}

fn tool_definitions() -> Value {
    let vault_arg = json!({ "type": "string", "description": "Registered vault name" });
    // Notes are addressed by path, never by title: one vault can hold many files
    // called README.md, and a title-addressed read would silently pick one.
    let path_arg = json!({
        "type": "string",
        "description": "Note path relative to the vault root, e.g. \"docs/API.md\". \
                        Always use a path from list_notes or search_notes — titles are \
                        not unique."
    });
    json!([
        {
            "name": "list_vaults",
            "description": "List every registered knowledge vault (name and path).",
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        },
        {
            "name": "list_notes",
            "description": "List every note in a vault as \"path\" (plus a [reference] marker for read-only notes pulled in from dependencies).",
            "inputSchema": {
                "type": "object",
                "properties": { "vault": vault_arg },
                "required": ["vault"]
            }
        },
        {
            "name": "read_note",
            "description": "Read the full markdown content of a note, addressed by its path.",
            "inputSchema": {
                "type": "object",
                "properties": { "vault": vault_arg, "path": path_arg },
                "required": ["vault", "path"]
            }
        },
        {
            "name": "save_note",
            "description": "Create or overwrite a note at a path. Use [[Note Title]] wikilinks to connect knowledge; use [[vault-name/Note Title]] to link across vaults. Reference notes (from scope.include) are read-only and will be refused.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "vault": vault_arg,
                    "path": path_arg,
                    "content": { "type": "string", "description": "Full markdown content" }
                },
                "required": ["vault", "path", "content"]
            }
        },
        {
            "name": "search_notes",
            "description": "Full-text search across notes. Handles Thai text with dictionary word segmentation (finds words mid-sentence). Omit vault to search every vault.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query (Thai or English)" },
                    "vault": vault_arg,
                    "limit": {
                        "type": "integer",
                        "description": format!(
                            "Maximum results in total, not per vault (default {DEFAULT_LIMIT}, max {}). \
                             Ask for fewer when you only need the best match.",
                            search::MAX_LIMIT
                        ),
                        "minimum": 1
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_links",
            "description": "Show a note's connections: forward links, backlinks, and backlinks from other vaults.",
            "inputSchema": {
                "type": "object",
                "properties": { "vault": vault_arg, "path": path_arg },
                "required": ["vault", "path"]
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

/// Tool arguments come from a model, so a path is untrusted input like any other.
fn checked_path(args: &Value) -> Result<&str> {
    let key = str_arg(args, "path")?;
    ops::validate_key(key).map_err(|m| anyhow::anyhow!(m))?;
    Ok(key)
}

fn resolve_vault(registry: &Registry, name: &str) -> Result<PathBuf> {
    registry
        .get(name)?
        .with_context(|| format!("vault \"{name}\" is not registered"))
}

fn tool_list_vaults() -> Result<String> {
    let vaults = Registry::open()?.list()?;
    if vaults.is_empty() {
        return Ok("no vaults registered — run `samong vault add <name> <path>` first".into());
    }
    Ok(vaults
        .into_iter()
        .map(|(name, path)| format!("{name}\t{}", path.display()))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn tool_list_notes(args: &Value) -> Result<String> {
    let root = resolve_vault(&Registry::open()?, str_arg(args, "vault")?)?;
    let notes = vault::list_notes(&root)?;
    if notes.is_empty() {
        return Ok("(vault is empty)".into());
    }
    // Paths, because that is what every other tool takes. Reference notes are
    // marked so the agent knows they cannot be written to.
    Ok(notes
        .into_iter()
        .map(|n| {
            if n.reference {
                format!("{} [reference]", n.key)
            } else {
                n.key
            }
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn tool_read_note(args: &Value) -> Result<String> {
    let key = checked_path(args)?;
    let root = resolve_vault(&Registry::open()?, str_arg(args, "vault")?)?;
    let path = ops::resolve_key(&root, key).map_err(|m| anyhow::anyhow!(m))?;
    if !path.is_file() {
        anyhow::bail!("note \"{key}\" does not exist");
    }
    fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
}

fn tool_save_note(args: &Value) -> Result<String> {
    let vault_name = str_arg(args, "vault")?;
    let key = checked_path(args)?;
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .context("missing required argument \"content\"")?;

    let root = resolve_vault(&Registry::open()?, vault_name)?;
    let path = ops::resolve_key(&root, key).map_err(|m| anyhow::anyhow!(m))?;
    let scope = crate::scope::Scope::load(&root)?;
    // Without this, saving to a vendored docs path would overwrite a
    // dependency's file, and the next install would erase what was just learned.
    if scope.is_reference(key) {
        anyhow::bail!(
            "cannot save \"{key}\": it is a read-only reference note from a scope.include \
             directory (it belongs to a dependency and would be erased on reinstall)"
        );
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    let report = indexer::reindex(&root, false)?;
    Ok(format!(
        "saved \"{key}\" in vault \"{vault_name}\" ({report})"
    ))
}

fn tool_search_notes(args: &Value) -> Result<String> {
    let query = str_arg(args, "query")?;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_LIMIT);
    let options = search::SearchOptions::with_limit(limit);

    let registry = Registry::open()?;
    let targets: Vec<(String, PathBuf)> = match args.get("vault").and_then(Value::as_str) {
        Some(name) if !name.is_empty() => vec![(name.to_string(), resolve_vault(&registry, name)?)],
        _ => registry.list()?,
    };

    // Each vault returns up to `limit` hits, so searching every vault could
    // otherwise hand back limit × vaults results — thousands of tokens the
    // caller pays for on every later turn. Rank them together and keep the
    // requested number in total.
    let mut hits = Vec::new();
    for (name, root) in targets {
        indexer::reindex(&root, false)?;
        for hit in search::query_with(&root, query, &options)? {
            hits.push((name.clone(), hit));
        }
    }
    hits.sort_by(|(_, a), (_, b)| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(options.limit.clamp(1, search::MAX_LIMIT));

    if hits.is_empty() {
        return Ok("no results".into());
    }
    let lines: Vec<String> = hits
        .into_iter()
        .map(|(name, hit)| {
            let snippet = hit
                .snippet
                .replace("<b>", "**")
                .replace("</b>", "**")
                .replace('\n', " ");
            // The path, not the title: this is what read_note takes, and it is
            // the only thing that tells two same-named notes apart.
            format!("{name}/{}: {snippet}", hit.key)
        })
        .collect();
    Ok(lines.join("\n"))
}

fn tool_get_links(args: &Value) -> Result<String> {
    let vault_name = str_arg(args, "vault")?;
    let key = checked_path(args)?;
    let registry = Registry::open()?;
    let root = resolve_vault(&registry, vault_name)?;
    indexer::reindex(&root, false)?;

    let title = crate::graph::title_from_key(key).unwrap_or_default();
    let (forward, backlinks) = {
        let graph = Graph::open(&root)?;
        // Forward links of *this* file. Each target is shown with the path it
        // resolves to, so the agent can read it without guessing.
        let mut forward = Vec::new();
        for target in graph.forward_links(key)? {
            let keys = graph.keys_for_title(&target)?;
            forward.push(match keys.len() {
                0 => format!("[[{target}]] -> (no such note)"),
                _ => format!("[[{target}]] -> {}", keys.join(", ")),
            });
        }
        let backlinks: Vec<String> = graph
            .backlinks(&title)?
            .into_iter()
            .filter(|source| source != key)
            .collect();
        (forward, backlinks)
    };
    let cross = ops::cross_vault_backlinks(&registry, vault_name, &title)?;

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
        assert_eq!(v["result"]["serverInfo"]["name"], "samong-mcp");
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
