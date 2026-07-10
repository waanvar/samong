//! Phase 4 acceptance: every REST endpoint answers correctly, and editing a
//! .md file directly on disk produces a WebSocket event.
//!
//! Everything runs in ONE test because the registry location comes from the
//! BANYAN_CONFIG_DIR environment variable, which is process-global.

use std::fs;
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::Value;

use banyan::registry::Registry;
use banyan::server::{self, AppState};

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
    std::env::set_var("BANYAN_CONFIG_DIR", &config);

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
    banyan::indexer::reindex(&work_canonical, false).unwrap();
    banyan::indexer::reindex(&ideas_canonical, false).unwrap();
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

    // ---- GET /api/vaults/{vault}/notes ----
    let (status, body) = get_json(&client, &format!("{base}/api/vaults/work/notes")).await;
    assert!(status.is_success());
    assert_eq!(body, serde_json::json!(["Local", "Source"]));
    let (status, _) = get_json(&client, &format!("{base}/api/vaults/nope/notes")).await;
    assert_eq!(status, reqwest::StatusCode::NOT_FOUND);

    // ---- GET /api/notes/{vault}/{title} ----
    let (status, body) = get_json(&client, &format!("{base}/api/notes/work/Source")).await;
    assert!(status.is_success());
    assert!(body["content"].as_str().unwrap().contains("[[Local]]"));
    let (status, _) = get_json(&client, &format!("{base}/api/notes/work/Missing")).await;
    assert_eq!(status, reqwest::StatusCode::NOT_FOUND);

    // ---- PUT /api/notes/{vault}/{title} (create) then GET it back ----
    let resp = client
        .put(format!("{base}/api/notes/work/FromApi"))
        .body("# FromApi\n\nsaved through the api with [[Local]]\n")
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let (status, body) = get_json(&client, &format!("{base}/api/notes/work/FromApi")).await;
    assert!(status.is_success());
    assert!(body["content"].as_str().unwrap().contains("saved through"));

    // path traversal is rejected
    let resp = client
        .put(format!("{base}/api/notes/work/..%5Cevil"))
        .body("nope")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    // ---- GET /api/notes/{vault}/{title}/links (incl. cross-vault) ----
    let (status, body) = get_json(&client, &format!("{base}/api/notes/work/Source/links")).await;
    assert!(status.is_success());
    let forward: Vec<&str> = body["forward"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(forward.contains(&"Local") && forward.contains(&"ideas/Target"));

    let (status, body) = get_json(&client, &format!("{base}/api/notes/ideas/Target/links")).await;
    assert!(status.is_success());
    assert_eq!(
        body["cross_vault_backlinks"],
        serde_json::json!(["work/Source"])
    );

    // ---- GET /api/search: Thai mid-sentence, per-vault and all-vaults ----
    let (status, body) = get_json(
        &client,
        &format!("{base}/api/search?q=%E0%B8%95%E0%B8%A5%E0%B8%B2%E0%B8%94%E0%B8%AB%E0%B8%A5%E0%B8%B1%E0%B8%81%E0%B8%97%E0%B8%A3%E0%B8%B1%E0%B8%9E%E0%B8%A2%E0%B9%8C&vault=ideas"),
    )
    .await;
    assert!(status.is_success(), "thai search failed: {body}");
    assert_eq!(body[0]["title"], "Target");

    let (status, body) = get_json(&client, &format!("{base}/api/search?q=plain")).await;
    assert!(status.is_success());
    let hits: Vec<(&str, &str)> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["vault"].as_str().unwrap(), v["title"].as_str().unwrap()))
        .collect();
    assert!(hits.contains(&("work", "Local")));

    // ---- GET /api/graph ----
    let (status, body) = get_json(&client, &format!("{base}/api/graph?vault=work")).await;
    assert!(status.is_success());
    let edges = body["edges"].as_array().unwrap();
    assert!(edges
        .iter()
        .any(|e| e["from"] == "Source" && e["to"] == "Local"));

    let (status, body) = get_json(&client, &format!("{base}/api/graph")).await;
    assert!(status.is_success());
    let edges = body["edges"].as_array().unwrap();
    assert!(edges
        .iter()
        .any(|e| e["from"] == "work/Source" && e["to"] == "ideas/Target"));
    assert!(body["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n == "ideas/Target"));

    // ---- DELETE /api/notes/{vault}/{title} reports dangling backlinks ----
    let resp = client
        .delete(format!("{base}/api/notes/work/Local"))
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
        .any(|v| v["title"] == "Fresh"));
}
