//! The MCP registry entry, kept honest against the rest of the repo.
//!
//! `server.json` is published to a registry other people's clients read. Its
//! version and bundle URL are edited by the release workflow, so the only thing
//! guarding the committed copy is this file — and the failure mode is quiet: a
//! stale entry points a client at an artefact that does not exist, and nothing in
//! a normal build notices.

use std::fs;

fn server_json() -> serde_json::Value {
    let raw = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/server.json"))
        .expect("server.json is at the repository root");
    serde_json::from_str(&raw).expect("server.json is valid JSON")
}

fn manifest_version() -> String {
    let raw = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    raw.lines()
        .find_map(|l| l.strip_prefix("version = \""))
        .and_then(|v| v.split('"').next())
        .expect("Cargo.toml declares a version")
        .to_string()
}

/// The registry name is what clients address the server by, and the namespace is
/// what GitHub OIDC proves ownership of. Getting it wrong means the publish is
/// rejected, or worse, succeeds under a name nobody expects.
#[test]
fn the_server_name_is_in_the_namespace_github_oidc_can_prove() {
    let doc = server_json();
    let name = doc["name"].as_str().expect("name is a string");
    assert_eq!(name, "io.github.waanvar/samong");
    assert_eq!(name.matches('/').count(), 1, "exactly one slash");
    assert!(
        name.starts_with("io.github.waanvar/"),
        "the namespace must match the GitHub account that publishes it"
    );
}

/// A bundle URL naming a version the crate is not at points clients somewhere
/// that will 404 the moment the two disagree.
#[test]
fn server_json_names_the_same_version_as_the_crate() {
    let doc = server_json();
    let version = manifest_version();
    assert_eq!(
        doc["version"].as_str(),
        Some(version.as_str()),
        "server.json version must track Cargo.toml"
    );
    let package = &doc["packages"][0];
    assert_eq!(package["version"].as_str(), Some(version.as_str()));
    let url = package["identifier"].as_str().expect("identifier is a URL");
    assert!(
        url.contains(&format!("/v{version}/")),
        "the bundle URL must name v{version}: {url}"
    );
}

/// Two rules the registry enforces at publish time, checked here instead of there.
#[test]
fn the_bundle_url_satisfies_what_the_registry_requires() {
    let doc = server_json();
    let package = &doc["packages"][0];
    assert_eq!(package["registryType"].as_str(), Some("mcpb"));
    assert_eq!(package["transport"]["type"].as_str(), Some("stdio"));

    let url = package["identifier"].as_str().unwrap();
    assert!(
        url.contains("mcp"),
        "the registry requires 'mcp' in the URL: {url}"
    );
    assert!(url.starts_with("https://github.com/waanvar/samong/releases/download/"));
    assert!(url.ends_with(".mcpb"));

    // Deliberately absent from the committed file: a hash checked into git would
    // be a claim about a file that has not been built. The release workflow adds
    // it, and `point-server-json.py` refuses anything that is not a digest.
    assert!(
        package.get("fileSha256").is_none(),
        "fileSha256 belongs to the release, not to the repository"
    );

    // v0.3.8 was rejected for this. The schema permits the field on any package;
    // the registry forbids it on mcpb specifically, so only a rule here catches it.
    assert!(
        package.get("registryBaseUrl").is_none(),
        "mcpb packages must carry the whole URL in identifier and no registryBaseUrl"
    );
}

/// Field limits the registry enforces. `validate-server-json.py` checks the whole
/// document against the published schema, but it needs the network; these are the
/// two that actually bit, so they fail here too, offline, in one second.
#[test]
fn the_fields_stay_within_the_registry_limits() {
    let doc = server_json();
    let description = doc["description"]
        .as_str()
        .expect("description is a string");
    assert!(
        description.chars().count() <= 100,
        "the registry rejects a description over 100 characters — this is {}          (v0.3.7 was rejected for exactly this)",
        description.chars().count()
    );
    assert!(!description.is_empty());

    let title = doc["title"].as_str().expect("title is a string");
    assert!(title.chars().count() <= 100 && !title.is_empty());

    let name = doc["name"].as_str().unwrap();
    assert!((3..=200).contains(&name.chars().count()));
}

/// The bundle manifest promises a tool list to clients that browse the registry.
/// It has to be the tools the server really exposes.
#[test]
fn the_bundle_manifest_lists_the_tools_the_mcp_server_actually_has() {
    let raw = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/packaging/mcpb/manifest.json"
    ))
    .expect("the bundle manifest is committed");
    let manifest: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    assert_eq!(manifest["server"]["type"].as_str(), Some("binary"));
    let overrides = &manifest["server"]["mcp_config"]["platform_overrides"];
    assert!(
        overrides["win32"]["command"].is_string(),
        "windows needs .exe"
    );
    assert!(
        overrides["darwin"]["command"].is_string(),
        "macOS needs the universal binary, not the linux one"
    );

    let declared: Vec<&str> = manifest["tools"]
        .as_array()
        .expect("tools is an array")
        .iter()
        .map(|t| t["name"].as_str().expect("tool has a name"))
        .collect();
    let mut declared_sorted = declared.clone();
    declared_sorted.sort_unstable();

    let mut actual = samong::mcp::tool_names();
    actual.sort_unstable();
    let actual: Vec<&str> = actual.iter().map(String::as_str).collect();

    assert_eq!(
        declared_sorted, actual,
        "the manifest's tool list has drifted from the server's"
    );
}
