//! Phase 30 — what happens when somebody double-clicks Samong.
//!
//! These drive the real `samong-app` binary with nothing but an empty config
//! directory and a fake home, because the first run is the only run that decides
//! whether a non-technical person ever sees a note. Everything it must do — find
//! a port, create a vault, serve, open — has to work with no arguments at all.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Launch the app the way Explorer or Finder would: no arguments.
///
/// `HOME`/`USERPROFILE` are redirected so the first-run vault lands in a
/// temporary directory instead of the developer's Documents folder — a test that
/// creates files in someone's real home is a test nobody runs twice. The port is
/// pinned per test because the launcher's real behaviour is to *join* a Samong it
/// finds on the usual port, which on a shared range would mean one test silently
/// joining another and both proving nothing.
fn launch(home: &Path, config: &Path, port: u16) -> Child {
    Command::new(env!("CARGO_BIN_EXE_samong-app"))
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("SAMONG_CONFIG_DIR", config)
        .env("SAMONG_PORT", port.to_string())
        .env("SAMONG_NO_OPEN", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the launcher should start")
}

/// A port nothing is on right now. Racy in principle; in practice the launcher
/// starts within milliseconds and would move up anyway if it lost the race.
fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Ask a port for `/api/vaults`, the way the launcher's own probe does.
fn get_vaults(port: u16) -> Option<String> {
    let address = format!("127.0.0.1:{port}");
    let mut stream =
        TcpStream::connect_timeout(&address.parse().unwrap(), Duration::from_millis(400)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream
        .write_all(
            format!("GET /api/vaults HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .ok()?;
    let mut response = Vec::new();
    stream.take(65536).read_to_end(&mut response).ok()?;
    Some(String::from_utf8_lossy(&response).to_string())
}

fn post(port: u16, path: &str) -> Option<String> {
    let address = format!("127.0.0.1:{port}");
    let mut stream =
        TcpStream::connect_timeout(&address.parse().unwrap(), Duration::from_millis(400)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    stream
        .write_all(
            format!(
                "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Length: 0\r\n\
                 Connection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .ok()?;
    let mut response = Vec::new();
    stream.take(65536).read_to_end(&mut response).ok()?;
    Some(String::from_utf8_lossy(&response).to_string())
}

/// Wait for the launcher to be answering on the port it was given.
fn wait_for_server(port: u16, deadline: Duration) -> Option<String> {
    let started = Instant::now();
    while started.elapsed() < deadline {
        if let Some(response) = get_vaults(port) {
            if response.starts_with("HTTP/1.1 200") && response.contains('[') {
                return Some(response);
            }
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    None
}

struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The whole of the first run, with nothing supplied and nothing asked.
#[test]
fn double_clicking_it_creates_a_vault_with_notes_and_serves_them() {
    let home = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join("Documents")).unwrap();

    let port = free_port();
    let running = Running(launch(home.path(), config.path(), port));
    let response =
        wait_for_server(port, Duration::from_secs(30)).expect("the launcher should end up serving");

    // A vault it invented itself, in a folder a person will find again.
    let vault = home.path().join("Documents").join("Samong");
    assert!(vault.is_dir(), "first run creates {}", vault.display());
    assert!(vault.join("Welcome to Samong.md").is_file());
    assert!(vault.join("How Samong works.md").is_file());

    // Registered, so the UI has something to show rather than an empty state.
    assert!(
        response.contains("notes"),
        "the vault is registered and served: {response}"
    );

    // The notes are indexed — the search box works on the very first run, which
    // is the thing a new arrival is most likely to try.
    let indexed = get_vaults(port).unwrap();
    assert!(indexed.starts_with("HTTP/1.1 200"));

    drop(running);
}

/// A second double-click must not start a second server. Two servers on one
/// vault would both index it and both fight the same redb lock.
#[test]
fn a_second_double_click_joins_the_running_one_instead_of_starting_another() {
    let home = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();

    let port = free_port();
    let running = Running(launch(home.path(), config.path(), port));
    wait_for_server(port, Duration::from_secs(30)).expect("first launch should serve");

    // The second launch should notice and exit on its own, quickly.
    let mut second = launch(home.path(), config.path(), port);
    let started = Instant::now();
    let status = loop {
        if let Some(status) = second.try_wait().unwrap() {
            break status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the second launch should exit rather than serve"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(status.success(), "it exits cleanly rather than serving too");

    // And the first is still the one serving.
    assert!(
        get_vaults(port).is_some(),
        "the original server is untouched"
    );
    drop(running);
}

/// A window that can be opened has to be closable the same way. Without this the
/// only way to stop a double-clicked Samong is the task manager.
#[test]
fn the_shutdown_endpoint_actually_stops_the_process() {
    let home = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();

    let port = free_port();
    let mut child = launch(home.path(), config.path(), port);
    wait_for_server(port, Duration::from_secs(30)).expect("launcher should serve");

    let response = post(port, "/api/shutdown").expect("shutdown should answer");
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "the browser is answered before the socket closes: {response}"
    );

    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "it stops cleanly, not by crashing");
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the process should exit after being told to stop"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
