//! Local REST + WebSocket API over the vault registry (Phase 4).
//!
//! Binds to 127.0.0.1 only — local-first, no auth. All redb/tantivy access is
//! serialized behind one mutex because redb allows a single open handle per
//! database file per process; a coarse lock is plenty for a single-user
//! local server.

use std::collections::HashSet;
use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use notify::{RecursiveMode, Watcher};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// The built web UI, baked into the binary so `cargo install` and the release
/// archives both ship a working UI with nothing to place alongside the exe.
/// Populated by `web/dist` at compile time (`cd web && npm run build` first);
/// an unbuilt tree yields an empty set and the server falls back to API-only.
#[derive(RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

fn has_embedded_ui() -> bool {
    WebAssets::get("index.html").is_some()
}

use crate::graph::Graph;
use crate::indexer;
use crate::registry::Registry;
use crate::search;
use crate::vault;

const WATCH_DEBOUNCE: Duration = Duration::from_millis(300);

pub struct AppState {
    /// Serializes every redb/tantivy/file operation (see module docs).
    io_lock: Mutex<()>,
    /// Change notifications fanned out to /ws clients.
    events: broadcast::Sender<String>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let (events, _) = broadcast::channel(64);
        Arc::new(Self {
            io_lock: Mutex::new(()),
            events,
        })
    }
}

#[derive(Serialize)]
struct ChangeEvent<'a> {
    vault: &'a str,
    indexed: usize,
    removed: usize,
}

fn broadcast_change(state: &AppState, vault: &str, indexed: usize, removed: usize) {
    let event = ChangeEvent {
        vault,
        indexed,
        removed,
    };
    // No receivers is fine — nobody is watching right now.
    let _ = state
        .events
        .send(serde_json::to_string(&event).expect("event serializes"));
}

// ---------- error plumbing ----------

enum ApiError {
    NotFound(String),
    BadRequest(String),
    Internal(anyhow::Error),
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(m) => (StatusCode::NOT_FOUND, m),
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            Self::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

type ApiResult<T> = std::result::Result<Json<T>, ApiError>;

/// Run a blocking vault operation under the global I/O lock.
async fn run_op<T, F>(state: Arc<AppState>, op: F) -> std::result::Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce() -> std::result::Result<T, ApiError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let _guard = state.io_lock.lock().expect("io lock poisoned");
        op()
    })
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("task panicked: {e}")))?
}

/// Takes an already-open registry: redb allows only one handle per file per
/// process, so each handler opens the registry exactly once.
fn resolve_vault(registry: &Registry, name: &str) -> std::result::Result<PathBuf, ApiError> {
    registry
        .get(name)?
        .ok_or_else(|| ApiError::NotFound(format!("vault \"{name}\" is not registered")))
}

/// Note titles come from URL segments; never let them escape the vault.
fn validate_title(title: &str) -> std::result::Result<(), ApiError> {
    crate::ops::validate_title(title).map_err(ApiError::BadRequest)
}

// ---------- REST handlers ----------

#[derive(Serialize)]
struct VaultInfo {
    name: String,
    path: String,
}

async fn list_vaults(State(state): State<Arc<AppState>>) -> ApiResult<Vec<VaultInfo>> {
    let vaults = run_op(state, || {
        let registry = Registry::open()?;
        Ok(registry.list()?)
    })
    .await?;
    Ok(Json(
        vaults
            .into_iter()
            .map(|(name, path)| VaultInfo {
                name,
                path: path.display().to_string(),
            })
            .collect(),
    ))
}

async fn list_notes(
    State(state): State<Arc<AppState>>,
    UrlPath(vault_name): UrlPath<String>,
) -> ApiResult<Vec<String>> {
    let titles = run_op(state, move || {
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        Ok(vault::list_notes(&root)?
            .into_iter()
            .map(|n| n.title)
            .collect::<Vec<_>>())
    })
    .await?;
    Ok(Json(titles))
}

#[derive(Serialize)]
struct NoteContent {
    title: String,
    content: String,
}

async fn get_note(
    State(state): State<Arc<AppState>>,
    UrlPath((vault_name, title)): UrlPath<(String, String)>,
) -> ApiResult<NoteContent> {
    let note = run_op(state, move || {
        validate_title(&title)?;
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        let note = vault::find_note(&root, &title)?
            .ok_or_else(|| ApiError::NotFound(format!("note \"{title}\" does not exist")))?;
        let content = fs::read_to_string(&note.path)
            .with_context(|| format!("reading {}", note.path.display()))?;
        Ok(NoteContent {
            title: note.title,
            content,
        })
    })
    .await?;
    Ok(Json(note))
}

async fn put_note(
    State(state): State<Arc<AppState>>,
    UrlPath((vault_name, title)): UrlPath<(String, String)>,
    body: String,
) -> ApiResult<serde_json::Value> {
    let state2 = Arc::clone(&state);
    let result = run_op(state, move || {
        validate_title(&title)?;
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        // Update in place if the note lives in a subfolder; create at the
        // vault root otherwise.
        let path = match vault::find_note(&root, &title)? {
            Some(existing) => existing.path,
            None => vault::note_path(&root, &title),
        };
        fs::write(&path, &body).with_context(|| format!("writing {}", path.display()))?;
        let report = indexer::reindex(&root, false)?;
        Ok((vault_name, report.indexed, report.removed))
    })
    .await?;
    broadcast_change(&state2, &result.0, result.1, result.2);
    Ok(Json(serde_json::json!({ "saved": true })))
}

async fn delete_note(
    State(state): State<Arc<AppState>>,
    UrlPath((vault_name, title)): UrlPath<(String, String)>,
) -> ApiResult<serde_json::Value> {
    let state2 = Arc::clone(&state);
    let (vault_name, dangling) = run_op(state, move || {
        validate_title(&title)?;
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        let note = vault::find_note(&root, &title)?
            .ok_or_else(|| ApiError::NotFound(format!("note \"{title}\" does not exist")))?;
        indexer::reindex(&root, false)?;
        let dangling: Vec<String> = {
            let graph = Graph::open(&root)?;
            let sources = graph
                .backlinks(&title)?
                .into_iter()
                .filter(|source_key| *source_key != note.key) // a self-link is not dangling
                .collect();
            crate::ops::keys_to_titles(sources)
        };
        fs::remove_file(&note.path).with_context(|| format!("deleting {}", note.path.display()))?;
        indexer::reindex(&root, false)?;
        Ok((vault_name, dangling))
    })
    .await?;
    broadcast_change(&state2, &vault_name, 0, 1);
    Ok(Json(
        serde_json::json!({ "deleted": true, "dangling_backlinks": dangling }),
    ))
}

#[derive(Serialize)]
struct LinksResponse {
    forward: Vec<String>,
    backlinks: Vec<String>,
    cross_vault_backlinks: Vec<String>,
}

async fn get_links(
    State(state): State<Arc<AppState>>,
    UrlPath((vault_name, title)): UrlPath<(String, String)>,
) -> ApiResult<LinksResponse> {
    let links = run_op(state, move || {
        validate_title(&title)?;
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        indexer::reindex(&root, false)?;
        let (forward, backlinks) = {
            let graph = Graph::open(&root)?;
            (
                graph.forward_links_for_title(&title)?,
                crate::ops::keys_to_titles(graph.backlinks(&title)?),
            )
        };

        // Same query-time federation as `banyan links --all-vaults`.
        let registry = Registry::open()?;
        let cross = crate::ops::cross_vault_backlinks(&registry, &vault_name, &title)?;
        Ok(LinksResponse {
            forward,
            backlinks,
            cross_vault_backlinks: cross,
        })
    })
    .await?;
    Ok(Json(links))
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
    vault: Option<String>,
}

#[derive(Serialize)]
struct SearchResult {
    vault: String,
    title: String,
    /// Vault-relative path of the hit. Titles repeat across directories, so
    /// this is what tells two same-named results apart.
    path: String,
    snippet: String,
}

async fn search_notes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> ApiResult<Vec<SearchResult>> {
    let results = run_op(state, move || {
        let registry = Registry::open()?;
        let targets: Vec<(String, PathBuf)> = match &params.vault {
            Some(name) => vec![(name.clone(), resolve_vault(&registry, name)?)],
            None => registry.list()?,
        };
        let mut out = Vec::new();
        for (name, root) in targets {
            indexer::reindex(&root, false)?;
            for hit in search::query(&root, &params.q)? {
                out.push(SearchResult {
                    vault: name.clone(),
                    title: hit.title,
                    path: hit.key,
                    snippet: hit.snippet,
                });
            }
        }
        Ok(out)
    })
    .await?;
    Ok(Json(results))
}

#[derive(Deserialize)]
struct GraphParams {
    vault: Option<String>,
}

#[derive(Serialize)]
struct GraphEdge {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct GraphResponse {
    nodes: Vec<String>,
    edges: Vec<GraphEdge>,
}

async fn get_graph(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GraphParams>,
) -> ApiResult<GraphResponse> {
    let response = run_op(state, move || {
        let registry = Registry::open()?;
        let (targets, qualify) = match &params.vault {
            Some(name) => (vec![(name.clone(), resolve_vault(&registry, name)?)], false),
            None => (registry.list()?, true),
        };
        let names: HashSet<String> = registry.list()?.into_iter().map(|(n, _)| n).collect();

        let mut nodes = HashSet::new();
        let mut edges = Vec::new();
        for (name, root) in &targets {
            indexer::reindex(root, false)?;
            for note in vault::list_notes(root)? {
                nodes.insert(if qualify {
                    format!("{name}/{}", note.title)
                } else {
                    note.title
                });
            }
            let vault_edges = {
                let graph = Graph::open(root)?;
                graph.all_edges()?
            };
            for (from, to) in vault_edges {
                // Nodes are titles (that is what a wikilink target names), so an
                // edge source has to be collapsed from its key or it would point
                // at a node that does not exist in the set above.
                let from = crate::graph::title_from_key(&from).unwrap_or(from);
                let (from, to) = if qualify {
                    let to = match vault::split_cross_vault(&to) {
                        Some((prefix, _)) if names.contains(prefix) => to.clone(),
                        _ => format!("{name}/{to}"),
                    };
                    (format!("{name}/{from}"), to)
                } else {
                    (from, to)
                };
                nodes.insert(from.clone());
                nodes.insert(to.clone());
                edges.push(GraphEdge { from, to });
            }
        }
        let mut nodes: Vec<String> = nodes.into_iter().collect();
        nodes.sort();
        edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
        Ok(GraphResponse { nodes, edges })
    })
    .await?;
    Ok(Json(response))
}

// ---------- WebSocket ----------

async fn ws_upgrade(State(state): State<Arc<AppState>>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| ws_client(socket, state))
}

async fn ws_client(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.events.subscribe();
    loop {
        match rx.recv().await {
            Ok(event) => {
                if socket.send(Message::Text(event.into())).await.is_err() {
                    return; // client disconnected
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}

// ---------- filesystem watcher (feeds /ws) ----------

fn event_vault<'a>(event: &notify::Event, vaults: &'a [(String, PathBuf)]) -> Option<&'a str> {
    for path in &event.paths {
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if path.components().any(|c| c.as_os_str() == vault::BRAIN_DIR) {
            continue; // our own index writes
        }
        for (name, root) in vaults {
            if path.starts_with(root) {
                return Some(name);
            }
        }
    }
    None
}

/// Watch every registered vault and broadcast an event after each reindex
/// that actually changed something. Runs on its own thread until the process
/// exits.
pub fn spawn_watcher(state: Arc<AppState>, vaults: Vec<(String, PathBuf)>) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(tx).context("creating filesystem watcher")?;
    for (_, root) in &vaults {
        watcher
            .watch(root, RecursiveMode::Recursive)
            .with_context(|| format!("watching {}", root.display()))?;
    }

    std::thread::spawn(move || {
        let _watcher = watcher; // keep alive on this thread
        loop {
            let Ok(event) = rx.recv() else {
                return;
            };
            let mut touched: HashSet<String> = HashSet::new();
            if let Ok(e) = &event {
                if let Some(name) = event_vault(e, &vaults) {
                    touched.insert(name.to_string());
                }
            }
            // Absorb the burst of events editors emit per save.
            let deadline = Instant::now() + WATCH_DEBOUNCE;
            while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                match rx.recv_timeout(remaining) {
                    Ok(Ok(e)) => {
                        if let Some(name) = event_vault(&e, &vaults) {
                            touched.insert(name.to_string());
                        }
                    }
                    Ok(Err(_)) => {}
                    Err(_) => break,
                }
            }
            for name in touched {
                let Some((_, root)) = vaults.iter().find(|(n, _)| *n == name) else {
                    continue;
                };
                let report = {
                    let _guard = state.io_lock.lock().expect("io lock poisoned");
                    indexer::reindex(root, false)
                };
                match report {
                    Ok(r) if r.indexed > 0 || r.removed > 0 => {
                        broadcast_change(&state, &name, r.indexed, r.removed);
                    }
                    Ok(_) => {} // change already indexed via the API
                    Err(err) => eprintln!("watch reindex failed for {name}: {err:#}"),
                }
            }
        }
    });
    Ok(())
}

// ---------- assembly ----------

/// API router plus the built web UI served from `ui_dir` on disk, with an
/// index.html fallback so the SPA handles its own navigation. Used to override
/// the baked-in UI during development (`--ui web/dist`).
pub fn router_with_ui(state: Arc<AppState>, ui_dir: &std::path::Path) -> Router {
    use tower_http::services::{ServeDir, ServeFile};
    let spa = ServeDir::new(ui_dir).fallback(ServeFile::new(ui_dir.join("index.html")));
    router(state).fallback_service(spa)
}

/// Serve one embedded asset, falling back to index.html for unknown paths so
/// the SPA owns client-side routing.
async fn serve_embedded(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let asset = WebAssets::get(path).or_else(|| WebAssets::get("index.html"));
    match asset {
        Some(file) => (
            [(header::CONTENT_TYPE, file.metadata.mimetype())],
            file.data,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "web UI not built").into_response(),
    }
}

/// API router plus the web UI baked into the binary.
pub fn router_with_embedded_ui(state: Arc<AppState>) -> Router {
    router(state).fallback(serve_embedded)
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/vaults", get(list_vaults))
        .route("/api/vaults/{vault}/notes", get(list_notes))
        .route(
            "/api/notes/{vault}/{title}",
            get(get_note).put(put_note).delete(delete_note),
        )
        .route("/api/notes/{vault}/{title}/links", get(get_links))
        .route("/api/search", get(search_notes))
        .route("/api/graph", get(get_graph))
        .route("/ws", get(ws_upgrade))
        .with_state(state)
}

/// Start the server on 127.0.0.1:port (local-first: never binds elsewhere).
///
/// The web UI is served from, in order of preference: an explicit `ui_dir`
/// on disk (dev override), then the UI baked into the binary, then API-only.
/// When `open_browser` is set, the default browser is pointed at the server
/// once it is listening.
pub async fn run(port: u16, ui_dir: Option<PathBuf>, open_browser: bool) -> Result<()> {
    let vaults = {
        // Scoped: handlers each open the registry themselves, and redb
        // allows only one live handle per file per process.
        let registry = Registry::open()?;
        registry.list()?
    };
    if vaults.is_empty() {
        eprintln!("warning: no vaults registered — use `banyan vault add <name> <path>` first");
    }
    for (name, root) in &vaults {
        let report = indexer::reindex(root, false)?;
        println!("vault \"{name}\": {report}");
    }

    // Best-effort update check off the hot path: never blocks startup or fails
    // if offline / no release yet.
    std::thread::spawn(crate::update::notify_if_outdated);

    let state = AppState::new();
    spawn_watcher(Arc::clone(&state), vaults)?;

    let app = match ui_dir.filter(|dir| dir.join("index.html").exists()) {
        Some(dir) => {
            println!("serving web UI from {}", dir.display());
            router_with_ui(state, &dir)
        }
        None if has_embedded_ui() => {
            println!("serving built-in web UI");
            router_with_embedded_ui(state)
        }
        None => {
            println!("web UI not built — serving API only (run `cd web && npm run build`)");
            router(state)
        }
    };

    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    let url = format!("http://{addr}");
    println!("banyan-server listening on {url} (Ctrl+C to stop)");
    if open_browser {
        // Launch off-thread so a slow browser start never delays serving.
        let url = url.clone();
        std::thread::spawn(move || {
            let _ = open::that(url);
        });
    }
    axum::serve(listener, app).await.context("serving")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tolerant of both environments: with a real `web/dist` build the
    /// embedded UI serves index.html; on a clean checkout (only .gitkeep)
    /// it reports the UI isn't built. Either way, no panic and the SPA
    /// fallback path is exercised.
    #[tokio::test]
    async fn embedded_ui_serves_index_or_reports_unbuilt() {
        let root = serve_embedded(Uri::from_static("/")).await;
        let spa_route = serve_embedded(Uri::from_static("/some/client/route")).await;
        if has_embedded_ui() {
            assert_eq!(root.status(), StatusCode::OK);
            // Unknown paths fall back to index.html, not 404.
            assert_eq!(spa_route.status(), StatusCode::OK);
        } else {
            assert_eq!(root.status(), StatusCode::NOT_FOUND);
        }
    }
}
