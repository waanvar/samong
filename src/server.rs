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
use axum::routing::{get, post};
use axum::{Json, Router};
use notify::{RecursiveMode, Watcher};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Notify};

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
use crate::ops;
use crate::registry::Registry;
use crate::scope::Scope;
use crate::search;
use crate::vault;

const WATCH_DEBOUNCE: Duration = Duration::from_millis(300);

pub struct AppState {
    /// Serializes every redb/tantivy/file operation (see module docs).
    io_lock: Mutex<()>,
    /// Change notifications fanned out to /ws clients.
    events: broadcast::Sender<String>,
    /// Raised by `POST /api/shutdown`.
    ///
    /// A server started from a desktop launcher has no terminal to press Ctrl+C
    /// in, so without this there is no way to stop it short of the task manager.
    /// "Open" implies "close"; a program that can only be opened is not finished.
    shutdown: Notify,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let (events, _) = broadcast::channel(64);
        Arc::new(Self {
            io_lock: Mutex::new(()),
            events,
            shutdown: Notify::new(),
        })
    }

    /// Resolves once someone has asked the server to stop.
    pub async fn stopped(&self) {
        self.shutdown.notified().await;
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

/// Path for human eyes: hide the `\\?\` verbatim prefix that canonicalize adds
/// on Windows. The CLI does the same — a path shown in the UI should look like
/// the one the user typed.
fn display_path(path: &std::path::Path) -> String {
    let shown = path.display().to_string();
    shown
        .strip_prefix(r"\\?\")
        .map(str::to_string)
        .unwrap_or(shown)
}

/// Resolve a note key from a URL wildcard segment.
fn resolve_key(vault: &std::path::Path, key: &str) -> std::result::Result<PathBuf, ApiError> {
    crate::ops::resolve_key(vault, key).map_err(ApiError::BadRequest)
}

/// One note as the API describes it. `key` is the address for every other call;
/// `title` is for display only, and is not unique.
#[derive(Serialize)]
struct NoteInfo {
    key: String,
    title: String,
    /// Came from `scope.include`: read-only, and absent on machines that do not
    /// have the source directory.
    reference: bool,
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
                path: display_path(&path),
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct NewVault {
    name: String,
    path: String,
}

/// Register a vault without leaving the browser.
///
/// Without this, a first-time user who downloads a binary and runs the server
/// lands on an empty page whose only remedy is a CLI command they have not read
/// about yet.
async fn add_vault(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewVault>,
) -> ApiResult<VaultInfo> {
    let info = run_op(state, move || {
        let registry = Registry::open()?;
        let canonical = registry
            .add(&body.name, std::path::Path::new(&body.path))
            .map_err(|err| ApiError::BadRequest(format!("{err:#}")))?;
        // Index it immediately so the UI has something to show.
        indexer::reindex(&canonical, false)?;
        Ok(VaultInfo {
            name: body.name,
            path: display_path(&canonical),
        })
    })
    .await?;
    Ok(Json(info))
}

async fn list_notes(
    State(state): State<Arc<AppState>>,
    UrlPath(vault_name): UrlPath<String>,
) -> ApiResult<Vec<NoteInfo>> {
    let notes = run_op(state, move || {
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        Ok(vault::list_notes(&root)?
            .into_iter()
            .map(|n| NoteInfo {
                key: n.key,
                title: n.title,
                reference: n.reference,
            })
            .collect::<Vec<_>>())
    })
    .await?;
    Ok(Json(notes))
}

#[derive(Serialize)]
struct NoteContent {
    key: String,
    title: String,
    content: String,
    reference: bool,
    /// Who published it, for a note that is not the reader's own. The reading
    /// pane is where a paragraph gets copied out of somebody else's vault, so
    /// it is where the licence has to be visible.
    source: Option<crate::provenance::Source>,
}

async fn get_note(
    State(state): State<Arc<AppState>>,
    UrlPath((vault_name, key)): UrlPath<(String, String)>,
) -> ApiResult<NoteContent> {
    let note = run_op(state, move || {
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        let path = resolve_key(&root, &key)?;
        if !path.is_file() {
            return Err(ApiError::NotFound(format!("note \"{key}\" does not exist")));
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let scope = Scope::load(&root)?;
        Ok(NoteContent {
            title: vault::title_from_path(&path).unwrap_or_default(),
            reference: scope.is_reference(&key),
            source: crate::provenance::Sources::for_scope(&scope)
                .of(&key)
                .cloned(),
            key,
            content,
        })
    })
    .await?;
    Ok(Json(note))
}

async fn put_note(
    State(state): State<Arc<AppState>>,
    UrlPath((vault_name, key)): UrlPath<(String, String)>,
    body: String,
) -> ApiResult<serde_json::Value> {
    let state2 = Arc::clone(&state);
    let result = run_op(state, move || {
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        let path = resolve_key(&root, &key)?;
        let scope = Scope::load(&root)?;
        if scope.is_reference(&key) {
            return Err(ApiError::BadRequest(format!(
                "cannot save \"{key}\": it is a read-only reference note from a \
                 scope.include directory (it belongs to a dependency and would be \
                 erased on reinstall)"
            )));
        }
        // The path may name a folder that does not exist yet.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::write(&path, &body).with_context(|| format!("writing {}", path.display()))?;
        let report = indexer::reindex(&root, false)?;
        // A note written outside the vault's scope (gitignored, say) is saved but
        // will never be searchable — say so rather than let it vanish quietly.
        let indexed = vault::list_notes(&root)?.iter().any(|n| n.key == key);
        Ok((vault_name, report.indexed, report.removed, indexed))
    })
    .await?;
    broadcast_change(&state2, &result.0, result.1, result.2);
    Ok(Json(
        serde_json::json!({ "saved": true, "indexed": result.3 }),
    ))
}

async fn delete_note(
    State(state): State<Arc<AppState>>,
    UrlPath((vault_name, key)): UrlPath<(String, String)>,
) -> ApiResult<serde_json::Value> {
    let state2 = Arc::clone(&state);
    let (vault_name, dangling) = run_op(state, move || {
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        let path = resolve_key(&root, &key)?;
        if !path.is_file() {
            return Err(ApiError::NotFound(format!("note \"{key}\" does not exist")));
        }
        if Scope::load(&root)?.is_reference(&key) {
            return Err(ApiError::BadRequest(format!(
                "cannot delete \"{key}\": it is a read-only reference note from a \
                 scope.include directory"
            )));
        }
        indexer::reindex(&root, false)?;
        let title = vault::title_from_path(&path).unwrap_or_default();
        let dangling: Vec<String> = {
            let graph = Graph::open(&root)?;
            let sources = graph
                .backlinks(&title)?
                .into_iter()
                .filter(|source_key| *source_key != key) // a self-link is not dangling
                .collect();
            crate::ops::keys_to_titles(sources)
        };
        fs::remove_file(&path).with_context(|| format!("deleting {}", path.display()))?;
        indexer::reindex(&root, false)?;
        Ok((vault_name, dangling))
    })
    .await?;
    broadcast_change(&state2, &vault_name, 0, 1);
    Ok(Json(
        serde_json::json!({ "deleted": true, "dangling_backlinks": dangling }),
    ))
}

/// A `[[target]]` as written, plus where it actually resolves. An empty `keys`
/// means the link dangles; more than one means the title is ambiguous.
#[derive(Serialize)]
struct ForwardLink {
    target: String,
    keys: Vec<String>,
}

#[derive(Serialize)]
struct NoteRef {
    key: String,
    title: String,
}

#[derive(Serialize)]
struct LinksResponse {
    forward: Vec<ForwardLink>,
    backlinks: Vec<NoteRef>,
    cross_vault_backlinks: Vec<String>,
}

async fn get_links(
    State(state): State<Arc<AppState>>,
    UrlPath((vault_name, key)): UrlPath<(String, String)>,
) -> ApiResult<LinksResponse> {
    let links = run_op(state, move || {
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        crate::ops::validate_key(&key).map_err(ApiError::BadRequest)?;
        indexer::reindex(&root, false)?;

        let title = crate::graph::title_from_key(&key).unwrap_or_default();
        let (forward, backlinks) = {
            let graph = Graph::open(&root)?;
            // Addressed by key, so this is *this* note's links — no union across
            // namesakes, which is what a title-addressed lookup had to do.
            let mut forward = Vec::new();
            for target in graph.forward_links(&key)? {
                let keys = graph.keys_for_title(&target)?;
                forward.push(ForwardLink { target, keys });
            }
            let backlinks = graph
                .backlinks(&title)?
                .into_iter()
                .filter(|source| *source != key)
                .map(|source| NoteRef {
                    title: crate::graph::title_from_key(&source).unwrap_or_default(),
                    key: source,
                })
                .collect();
            (forward, backlinks)
        };

        // Same query-time federation as `samong links --all-vaults`.
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

#[derive(Serialize)]
struct IncludeRootStatus {
    path: String,
    present: bool,
}

/// Everything `samong doctor` prints, for a browser.
///
/// Without this the web UI can show a vault with four notes and no way to learn
/// that ninety more were skipped, or that a `scope.include` directory is missing
/// on this machine — the user would conclude search is broken.
#[derive(Serialize)]
struct DoctorResponse {
    vault: String,
    /// What the vault says about itself, for a vault that came from someone else.
    /// Every field is optional and usually empty on a personal vault.
    manifest: Manifest,
    notes_dir: String,
    follow_gitignore: bool,
    include_roots: Vec<IncludeRootStatus>,
    project_notes: usize,
    reference_notes: usize,
    skipped: usize,
    skipped_dependency: usize,
    skipped_by_dir: Vec<(String, usize)>,
    truncated: bool,
    /// Titles shared by several notes where at least one is a project note.
    ambiguous_titles: Vec<AmbiguousTitle>,
    /// Collisions confined to reference notes: normal for vendored docs.
    reference_only_collisions: usize,
    /// Semantic search coverage, or `null` in a build without the feature.
    ///
    /// Nullable rather than zeroed so the UI can tell "this build cannot do
    /// semantic search" apart from "it can, and nothing is embedded yet" — two
    /// situations with completely different answers.
    embeddings: Option<EmbeddingStatus>,
}

#[derive(Serialize)]
struct EmbeddingStatus {
    /// `None` when a store exists but was never claimed by a model.
    model: Option<String>,
    notes: usize,
    /// Project notes with no vector: what `samong embed` would fix.
    missing_project: usize,
    /// Reference notes with no vector — the default, not a problem.
    missing_reference: usize,
}

/// Coverage for the doctor response.
#[cfg(feature = "semantic")]
fn embedding_status(scope: &Scope) -> Result<Option<EmbeddingStatus>, anyhow::Error> {
    let vault = scope.root();
    if !crate::vectors::exists(vault) {
        return Ok(Some(EmbeddingStatus {
            model: None,
            notes: 0,
            missing_project: vault::list_notes_in(scope)?
                .iter()
                .filter(|n| !n.reference)
                .count(),
            missing_reference: 0,
        }));
    }
    let store = crate::vectors::Store::open(vault)?;
    let stored = store.stored_hashes()?;
    let notes = vault::list_notes_in(scope)?;
    Ok(Some(EmbeddingStatus {
        model: store.meta()?.map(|(name, dim)| format!("{name} ({dim}d)")),
        notes: stored.len(),
        missing_project: notes
            .iter()
            .filter(|n| !n.reference && !stored.contains_key(&n.key))
            .count(),
        missing_reference: notes
            .iter()
            .filter(|n| n.reference && !stored.contains_key(&n.key))
            .count(),
    }))
}

#[cfg(not(feature = "semantic"))]
fn embedding_status(_scope: &Scope) -> Result<Option<EmbeddingStatus>, anyhow::Error> {
    Ok(None)
}

#[derive(Serialize)]
struct Manifest {
    description: Option<String>,
    version: Option<String>,
    license: Option<String>,
    source: Option<String>,
}

#[derive(Serialize)]
struct AmbiguousTitle {
    title: String,
    keys: Vec<String>,
}

async fn get_doctor(
    State(state): State<Arc<AppState>>,
    UrlPath(vault_name): UrlPath<String>,
) -> ApiResult<DoctorResponse> {
    let report = run_op(state, move || {
        let root = resolve_vault(&Registry::open()?, &vault_name)?;
        let scope = Scope::load(&root)?;
        let audit = scope.audit()?;
        indexer::reindex_in(&scope, false)?;

        let duplicates = {
            let graph = Graph::open(&root)?;
            graph.duplicate_titles()?
        };
        let (own, reference_only): (Vec<_>, Vec<_>) = duplicates
            .into_iter()
            .partition(|(_, keys)| keys.iter().any(|key| !scope.is_reference(key)));

        Ok(DoctorResponse {
            vault: display_path(&root),
            manifest: Manifest {
                description: scope.config().vault.description.clone(),
                version: scope.config().vault.version.clone(),
                license: scope.config().vault.license.clone(),
                source: scope.config().vault.source.clone(),
            },
            notes_dir: scope.config().scope.notes_dir.clone(),
            follow_gitignore: scope.config().scope.follow_gitignore,
            include_roots: scope
                .include_roots()
                .iter()
                .map(|r| IncludeRootStatus {
                    path: r.declared.clone(),
                    present: r.present,
                })
                .collect(),
            project_notes: audit.included - audit.reference,
            reference_notes: audit.reference,
            skipped: audit.skipped,
            skipped_dependency: audit.skipped_dependency,
            skipped_by_dir: audit.skipped_by_dir,
            truncated: audit.truncated,
            ambiguous_titles: own
                .into_iter()
                .map(|(title, keys)| AmbiguousTitle { title, keys })
                .collect(),
            reference_only_collisions: reference_only.len(),
            embeddings: embedding_status(&scope)?,
        })
    })
    .await?;
    Ok(Json(report))
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
    vault: Option<String>,
    /// Maximum hits per vault. Clamped to `search::MAX_LIMIT`.
    limit: Option<usize>,
}

#[derive(Serialize)]
struct SearchResult {
    vault: String,
    title: String,
    /// Vault-relative path of the hit. Titles repeat across directories, so
    /// this is what tells two same-named results apart.
    path: String,
    snippet: String,
    /// Who published this note, when it was not the reader. `null` for their
    /// own notes — a nullable field rather than a `reference: bool`, because
    /// "somebody else's" and "whose, under what licence" are what a reader
    /// about to quote it needs, and only one of those is a flag.
    source: Option<crate::provenance::Source>,
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
        let options = match params.limit {
            Some(limit) => search::SearchOptions::with_limit(limit),
            None => search::SearchOptions::default(),
        };
        let mut out = Vec::new();
        for (name, root) in targets {
            indexer::reindex(&root, false)?;
            for hit in ops::search_vault(&root, &params.q, &options)? {
                out.push(SearchResult {
                    vault: name.clone(),
                    title: hit.title,
                    path: hit.key,
                    snippet: hit.snippet,
                    source: hit.source,
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
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

/// A node is a *file*, addressed by `id` (the key, qualified with the vault name
/// in all-vaults mode) and labelled with its title.
///
/// Nodes used to be titles, which silently merged every file sharing one — a
/// vendored docs tree can hold forty pages called `index`, and they all became a
/// single blob in the middle of the graph.
#[derive(Serialize)]
struct GraphNode {
    id: String,
    label: String,
    /// A wikilink target that resolves to no note: drawn as a stub, not a file.
    missing: bool,
    reference: bool,
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

        let mut nodes: std::collections::HashMap<String, GraphNode> = Default::default();
        let mut edges = Vec::new();
        for (name, root) in &targets {
            let qualified = |id: &str| {
                if qualify {
                    format!("{name}/{id}")
                } else {
                    id.to_string()
                }
            };
            let scope = Scope::load(root)?;
            indexer::reindex_in(&scope, false)?;

            for note in vault::list_notes_in(&scope)? {
                let id = qualified(&note.key);
                nodes.insert(
                    id.clone(),
                    GraphNode {
                        id,
                        label: note.title,
                        missing: false,
                        reference: note.reference,
                    },
                );
            }

            let graph = Graph::open(root)?;
            for (from_key, target) in graph.all_edges()? {
                let from = qualified(&from_key);
                // Resolve the raw wikilink target to the file(s) it names, so an
                // edge lands on a real node instead of a title.
                let resolved = graph.keys_for_title(&target)?;
                if resolved.is_empty() {
                    // Either a cross-vault reference or a genuinely broken link.
                    let to = match vault::split_cross_vault(&target) {
                        Some((prefix, _)) if names.contains(prefix) => target.clone(),
                        _ => qualified(&target),
                    };
                    nodes.entry(to.clone()).or_insert_with(|| GraphNode {
                        id: to.clone(),
                        label: crate::graph::title_from_key(&target).unwrap_or(target.clone()),
                        missing: true,
                        reference: false,
                    });
                    edges.push(GraphEdge { from, to });
                    continue;
                }
                for key in resolved {
                    edges.push(GraphEdge {
                        from: from.clone(),
                        to: qualified(&key),
                    });
                }
            }
        }
        let mut nodes: Vec<GraphNode> = nodes.into_values().collect();
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
        edges.dedup_by(|a, b| a.from == b.from && a.to == b.to);
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

/// Stop the server.
///
/// `notify_one` rather than `notify_waiters`: it stores the permit if the serve
/// loop is not yet awaiting, so a shutdown asked for in the first instants after
/// startup cannot be dropped on the floor.
async fn shutdown(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    state.shutdown.notify_one();
    Json(serde_json::json!({ "stopping": true }))
}

/// API router plus the web UI baked into the binary.
pub fn router_with_embedded_ui(state: Arc<AppState>) -> Router {
    router(state).fallback(serve_embedded)
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/vaults", get(list_vaults).post(add_vault))
        .route("/api/vaults/{vault}/notes", get(list_notes))
        .route("/api/vaults/{vault}/doctor", get(get_doctor))
        // Notes are addressed by their vault-relative path, so the last segment
        // is a wildcard. Axum requires a wildcard to end the pattern, which is
        // why links live under their own prefix rather than `.../{path}/links`.
        .route(
            "/api/notes/{vault}/{*path}",
            get(get_note).put(put_note).delete(delete_note),
        )
        .route("/api/links/{vault}/{*path}", get(get_links))
        .route("/api/search", get(search_notes))
        .route("/api/graph", get(get_graph))
        .route("/api/shutdown", post(shutdown))
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
        eprintln!("warning: no vaults registered — use `samong vault add <name> <path>` first");
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
    // Kept before the router consumes `state`.
    let stopping = Arc::clone(&state);

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
    println!("samong-server listening on {url} (Ctrl+C to stop)");
    if open_browser {
        // Launch off-thread so a slow browser start never delays serving.
        let url = url.clone();
        std::thread::spawn(move || {
            let _ = open::that(url);
        });
    }
    // Graceful, so the browser gets its answer to `POST /api/shutdown` before
    // the socket closes — otherwise the page reports a network error for a
    // request that in fact did exactly what was asked.
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { stopping.stopped().await })
        .await
        .context("serving")?;
    println!("stopped");
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
