//! Phase 4 acceptance: every REST endpoint answers correctly, and editing a
//! .md file directly on disk produces a WebSocket event.
//!
//! Since Phase 14 the API addresses notes by their vault-relative *path*, not by
//! title — a vault can hold many files called `README.md`, and a title-addressed
//! API could only ever reach one of them.
//!
//! Everything runs in ONE test because the registry location comes from the
//! SAMONG_CONFIG_DIR environment variable, which is process-global.

use std::fs;
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::Value;

use samong::registry::Registry;
use samong::server::{self, AppState};

async fn get_json(client: &reqwest::Client, url: &str) -> (reqwest::StatusCode, Value) {
    let resp = client.get(url).send().await.unwrap();
    let status = resp.status();
    let body: Value = resp.json().await.unwrap();
    (status, body)
}

#[tokio::test(flavor = "multi_thread")]
async fn rest_endpoints_and_websocket_events() {
    // ---- fixture: registry + two cross-linked vaults ----
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    let work = root.path().join("work");
    let ideas = root.path().join("ideas");
    fs::create_dir_all(&work).unwrap();
    fs::create_dir_all(&ideas).unwrap();
    std::env::set_var("SAMONG_CONFIG_DIR", &config);

    fs::write(
        work.join("Source.md"),
        "# Source\n\ncross [[ideas/Target]] and local [[Local]]\n",
    )
    .unwrap();
    fs::write(work.join("Local.md"), "# Local\n\nplain content\n").unwrap();
    fs::write(
        ideas.join("Target.md"),
        "# Target\n\nโน้ตปลายทางมีคำว่าตลาดหลักทรัพย์อยู่กลางประโยค\n",
    )
    .unwrap();

    let registry = Registry::open().unwrap();
    let work_canonical = registry.add("work", &work).unwrap();
    let ideas_canonical = registry.add("ideas", &ideas).unwrap();
    samong::indexer::reindex(&work_canonical, false).unwrap();
    samong::indexer::reindex(&ideas_canonical, false).unwrap();
    drop(registry); // release registry.redb before the server opens it

    // ---- boot the server on an ephemeral local port ----
    let state = AppState::new();
    server::spawn_watcher(
        Arc::clone(&state),
        vec![
            ("work".to_string(), work_canonical.clone()),
            ("ideas".to_string(), ideas_canonical.clone()),
        ],
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, server::router(state)).into_future());
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    // ---- GET /api/vaults ----
    let (status, body) = get_json(&client, &format!("{base}/api/vaults")).await;
    assert!(status.is_success());
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["ideas", "work"]);

    // ---- GET /api/vaults/{vault}/notes: keys, titles, reference flag ----
    let (status, body) = get_json(&client, &format!("{base}/api/vaults/work/notes")).await;
    assert!(status.is_success());
    let listed: Vec<(&str, &str, bool)> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["key"].as_str().unwrap(),
                v["title"].as_str().unwrap(),
                v["reference"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        vec![("Local.md", "Local", false), ("Source.md", "Source", false)]
    );
    let (status, _) = get_json(&client, &format!("{base}/api/vaults/nope/notes")).await;
    assert_eq!(status, reqwest::StatusCode::NOT_FOUND);

    // ---- GET /api/notes/{vault}/{*path} ----
    let (status, body) = get_json(&client, &format!("{base}/api/notes/work/Source.md")).await;
    assert!(status.is_success());
    assert!(body["content"].as_str().unwrap().contains("[[Local]]"));
    assert_eq!(body["key"], "Source.md");
    assert_eq!(body["reference"], false);
    let (status, _) = get_json(&client, &format!("{base}/api/notes/work/Missing.md")).await;
    assert_eq!(status, reqwest::StatusCode::NOT_FOUND);

    // ---- PUT: a path addresses a subdirectory directly, parents are created ----
    let resp = client
        .put(format!("{base}/api/notes/work/notes/FromApi.md"))
        .body("# FromApi\n\nsaved through the api with [[Local]]\n")
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let saved: Value = resp.json().await.unwrap();
    assert_eq!(saved["saved"], true);
    assert_eq!(saved["indexed"], true, "a saved note must be searchable");
    let (status, body) =
        get_json(&client, &format!("{base}/api/notes/work/notes/FromApi.md")).await;
    assert!(status.is_success());
    assert!(body["content"].as_str().unwrap().contains("saved through"));

    // Every way out of the vault is refused, and so is a non-.md path.
    for bad in [
        "..%2Fevil.md",
        "..%5Cevil.md",
        "docs%2F..%2F..%2Fevil.md",
        "Plain",
    ] {
        let resp = client
            .put(format!("{base}/api/notes/work/{bad}"))
            .body("nope")
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "{bad} should be rejected"
        );
    }

    // ---- GET /api/links/{vault}/{*path} (incl. cross-vault) ----
    let (status, body) = get_json(&client, &format!("{base}/api/links/work/Source.md")).await;
    assert!(status.is_success());
    let forward = body["forward"].as_array().unwrap();
    // Each target says where it resolves, so a client never has to guess.
    let local = forward
        .iter()
        .find(|f| f["target"] == "Local")
        .expect("forward link to Local");
    assert_eq!(local["keys"], serde_json::json!(["Local.md"]));
    let cross = forward
        .iter()
        .find(|f| f["target"] == "ideas/Target")
        .expect("cross-vault forward link");
    assert_eq!(
        cross["keys"],
        serde_json::json!([]),
        "a cross-vault target resolves to no key in this vault"
    );

    let (status, body) = get_json(&client, &format!("{base}/api/links/ideas/Target.md")).await;
    assert!(status.is_success());
    assert_eq!(
        body["cross_vault_backlinks"],
        serde_json::json!(["work/Source"])
    );

    // ---- GET /api/vaults/{vault}/doctor ----
    let (status, body) = get_json(&client, &format!("{base}/api/vaults/work/doctor")).await;
    assert!(status.is_success(), "doctor failed: {body}");
    assert_eq!(body["project_notes"], 3, "Source, Local, notes/FromApi");
    assert_eq!(body["reference_notes"], 0);
    assert_eq!(body["follow_gitignore"], true);
    assert_eq!(body["ambiguous_titles"], serde_json::json!([]));

    // ---- GET /api/search: Thai mid-sentence, per-vault and all-vaults ----
    let (status, body) = get_json(
        &client,
        &format!("{base}/api/search?q=%E0%B8%95%E0%B8%A5%E0%B8%B2%E0%B8%94%E0%B8%AB%E0%B8%A5%E0%B8%B1%E0%B8%81%E0%B8%97%E0%B8%A3%E0%B8%B1%E0%B8%9E%E0%B8%A2%E0%B9%8C&vault=ideas"),
    )
    .await;
    assert!(status.is_success(), "thai search failed: {body}");
    assert_eq!(body[0]["title"], "Target");
    assert_eq!(body[0]["path"], "Target.md");

    let (status, body) = get_json(&client, &format!("{base}/api/search?q=plain")).await;
    assert!(status.is_success());
    let hits: Vec<(&str, &str)> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["vault"].as_str().unwrap(), v["path"].as_str().unwrap()))
        .collect();
    assert!(hits.contains(&("work", "Local.md")));

    // ---- GET /api/graph: nodes are files, not titles ----
    let (status, body) = get_json(&client, &format!("{base}/api/graph?vault=work")).await;
    assert!(status.is_success());
    let edges = body["edges"].as_array().unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e["from"] == "Source.md" && e["to"] == "Local.md"),
        "edges join files, not titles: {edges:?}"
    );
    let nodes = body["nodes"].as_array().unwrap();
    let source = nodes
        .iter()
        .find(|n| n["id"] == "Source.md")
        .expect("Source.md node");
    assert_eq!(source["label"], "Source");
    assert_eq!(source["missing"], false);

    let (status, body) = get_json(&client, &format!("{base}/api/graph")).await;
    assert!(status.is_success());
    let edges = body["edges"].as_array().unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e["from"] == "work/Source.md" && e["to"] == "ideas/Target"),
        "cross-vault edge keeps the vault-qualified target: {edges:?}"
    );
    assert!(body["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["id"] == "ideas/Target.md"));

    // ---- POST /api/vaults: register a vault without touching a terminal ----
    let fresh = root.path().join("fresh");
    fs::create_dir_all(&fresh).unwrap();
    fs::write(fresh.join("Hello.md"), "# Hello\n\nbrand new vault\n").unwrap();
    let resp = client
        .post(format!("{base}/api/vaults"))
        .json(&serde_json::json!({ "name": "fresh", "path": fresh.to_string_lossy() }))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "POST /api/vaults failed");
    let (status, body) = get_json(&client, &format!("{base}/api/vaults/fresh/notes")).await;
    assert!(status.is_success());
    assert_eq!(body[0]["key"], "Hello.md", "the new vault was indexed too");

    // A duplicate name, or a path that does not exist, is a client error.
    for bad in [
        serde_json::json!({ "name": "fresh", "path": fresh.to_string_lossy() }),
        serde_json::json!({ "name": "nowhere", "path": "/definitely/not/here" }),
    ] {
        let resp = client
            .post(format!("{base}/api/vaults"))
            .json(&bad)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST, "{bad}");
    }

    // ---- DELETE /api/notes/{vault}/{*path} reports dangling backlinks ----
    let resp = client
        .delete(format!("{base}/api/notes/work/Local.md"))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["deleted"], true);
    let dangling = body["dangling_backlinks"].as_array().unwrap();
    assert!(
        dangling.iter().any(|s| s == "Source"),
        "Source still links to Local: {dangling:?}"
    );

    // ---- acceptance: direct .md edit fires a WebSocket event ----
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
        .await
        .unwrap();

    fs::write(
        ideas.join("Fresh.md"),
        "# Fresh\n\nwritten directly on disk\n",
    )
    .unwrap();

    let msg = tokio::time::timeout(Duration::from_secs(15), ws.next())
        .await
        .expect("websocket event within 15s")
        .expect("stream open")
        .expect("frame ok");
    let event: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(event["vault"], "ideas");
    assert!(event["indexed"].as_u64().unwrap() >= 1);

    // The watcher also indexed it: the new note is now searchable.
    let (status, body) = get_json(&client, &format!("{base}/api/search?q=directly")).await;
    assert!(status.is_success());
    assert!(body
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["path"] == "Fresh.md"));
}
