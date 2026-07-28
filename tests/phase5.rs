//! Phase 5: the server serves the built SPA alongside the API.

use std::fs;
use std::future::IntoFuture;

use samong::server::{self, AppState};

#[tokio::test(flavor = "multi_thread")]
async fn serves_spa_with_index_fallback_and_api() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    std::env::set_var("SAMONG_CONFIG_DIR", &config);

    // Fake built UI.
    let ui = root.path().join("dist");
    fs::create_dir_all(ui.join("assets")).unwrap();
    fs::write(
        ui.join("index.html"),
        "<!doctype html><title>Samong</title>",
    )
    .unwrap();
    fs::write(ui.join("assets").join("app.js"), "console.log('samong')").unwrap();

    let state = AppState::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, server::router_with_ui(state, &ui)).into_future());
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    // Static index + asset.
    let index = client.get(&base).send().await.unwrap();
    assert!(index.status().is_success());
    assert!(index.text().await.unwrap().contains("Samong"));

    let asset = client
        .get(format!("{base}/assets/app.js"))
        .send()
        .await
        .unwrap();
    assert!(asset.status().is_success());

    // Unknown paths fall back to index.html (SPA routing).
    let fallback = client
        .get(format!("{base}/some/client/route"))
        .send()
        .await
        .unwrap();
    assert!(fallback.status().is_success());
    assert!(fallback.text().await.unwrap().contains("Samong"));

    // The API still answers on the same port.
    let api = client
        .get(format!("{base}/api/vaults"))
        .send()
        .await
        .unwrap();
    assert!(api.status().is_success());
}
