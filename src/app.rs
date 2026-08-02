//! Opening Samong by double-clicking it.
//!
//! Everything else in this crate assumes a terminal. That assumption quietly
//! decided who Samong is for: the people whose notes most need to stay on their
//! own machine — lawyers, doctors, researchers, anyone holding somebody else's
//! confidences — are not the people who will type `samong vault add`. A product
//! whose only door is a command line has already chosen its audience.
//!
//! So this module is the whole first run, with nothing to answer:
//!
//! 1. If Samong is **already serving**, point the browser at it. A second
//!    double-click must not start a second server — it should feel like
//!    switching to a window that is already open.
//! 2. If the port is taken by **something else** (3000 is the most contested
//!    port in software), move up rather than fail.
//! 3. If **no vault is registered**, make one, with notes in it. A first run
//!    that lands on an empty screen has explained nothing.
//!
//! # Why a default folder rather than a folder picker
//!
//! A picker would need a native file dialog — a dependency that pulls GTK on
//! Linux and would have to be right on three platforms before anyone sees a
//! note. The first run is not the moment to ask a question anyway: someone who
//! just double-clicked an unfamiliar app has no basis to answer "where should
//! your notes live?". They get a sensible folder, and the existing "add vault"
//! flow is still there for people who already know where their notes are.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::registry::Registry;

/// Where `samong-server` listens by default, and so where the launcher looks
/// first.
pub const DEFAULT_PORT: u16 = 3000;
/// Ports tried after the first one is found busy by something that is not us.
pub const PORT_ATTEMPTS: u16 = 12;
/// Name the first vault is registered under — the `[[notes/...]]` link prefix.
const DEFAULT_VAULT_NAME: &str = "notes";

/// Folder the first vault is created in.
///
/// `Documents` when the machine has one, because that is where a person who
/// does not think in filesystems will look for it later, and because a folder
/// under `~` starting with nothing recognisable is a folder that gets deleted
/// during a tidy-up.
pub fn default_vault_dir(home: &Path) -> PathBuf {
    let documents = home.join("Documents");
    if documents.is_dir() {
        documents.join("Samong")
    } else {
        home.join("Samong")
    }
}

/// What the launcher did about the absence of a vault, for reporting.
pub struct Created {
    pub name: String,
    pub path: PathBuf,
}

/// Make sure this machine has at least one vault, creating one if not.
///
/// Returns `None` when a vault was already registered — the overwhelmingly
/// common case, and one that must touch nothing.
pub fn ensure_a_vault(home: &Path) -> Result<Option<Created>> {
    {
        // Scoped: redb allows one live handle per file per process, and the
        // server opens the registry again the moment it starts.
        let registry = Registry::open()?;
        if !registry.list()?.is_empty() {
            return Ok(None);
        }
    }

    let path = default_vault_dir(home);
    std::fs::create_dir_all(&path)
        .with_context(|| format!("creating a first vault at {}", path.display()))?;
    write_welcome_notes(&path)?;

    let registry = Registry::open()?;
    registry.add(DEFAULT_VAULT_NAME, &path)?;
    Ok(Some(Created {
        name: DEFAULT_VAULT_NAME.to_string(),
        path,
    }))
}

/// Two notes, not one, and linked to each other.
///
/// The first thing a new arrival sees is the graph. One note draws a single dot
/// that demonstrates nothing; two notes and a link draw the actual idea of the
/// program. And the second note is reachable only by following the link, which
/// is the one interaction worth learning.
///
/// Existing files are never overwritten: the folder may be one the person
/// already had.
fn write_welcome_notes(vault: &Path) -> Result<()> {
    let notes: [(&str, &str); 2] = [
        (
            "Welcome to Samong.md",
            "# Welcome to Samong\n\n\
             These notes are plain Markdown files in a folder on this computer. \
             Nothing here is uploaded anywhere, and you can open the same folder \
             in any other editor.\n\n\
             Type a title in the search box at the top to make a new note. Type \
             `[[` inside a note to link it to another one — that is what draws \
             the map.\n\n\
             Next: [[How Samong works]]\n",
        ),
        (
            "How Samong works.md",
            "# How Samong works\n\n\
             You just followed a link, which is the whole idea.\n\n\
             - **Search finds words inside notes**, not just titles — including \
             Thai, which has no spaces between words.\n\
             - **The map** shows every note and every link. Bigger circles are \
             notes more things point at.\n\
             - **Everything stays here.** This folder is yours; back it up the \
             way you back up any other folder.\n\n\
             Back: [[Welcome to Samong]]\n",
        ),
    ];
    for (name, body) in notes {
        let path = vault.join(name);
        if !path.exists() {
            std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
        }
    }
    Ok(())
}

/// Is a Samong server already answering on this port?
///
/// Asks, rather than assuming: port 3000 belongs to whatever dev server was
/// started last, and opening a browser at somebody else's app would be worse
/// than any error message. A JSON array from `/api/vaults` is the cheapest
/// answer that no ordinary web server gives by accident — a static server or a
/// bundler returns `text/html` for an unknown path.
pub fn samong_is_serving(port: u16) -> bool {
    probe(port).unwrap_or(false)
}

fn probe(port: u16) -> Result<bool> {
    let address = format!("127.0.0.1:{port}");
    let mut stream = TcpStream::connect_timeout(
        &address.parse().context("localhost address")?,
        Duration::from_millis(400),
    )?;
    stream.set_read_timeout(Some(Duration::from_millis(700)))?;
    stream.write_all(
        format!("GET /api/vaults HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )?;
    let mut response = Vec::new();
    // Bounded: a wrong guess about what is on this port must not read forever.
    stream.take(8192).read_to_end(&mut response)?;
    let text = String::from_utf8_lossy(&response);
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_ref(), ""));
    let head = head.to_ascii_lowercase();
    Ok(head.starts_with("http/1.1 200")
        && head.contains("application/json")
        && body.trim_start().starts_with('['))
}

/// A port to serve on, and whether Samong is already there.
pub enum Where {
    /// Already running — open the browser at it and leave it alone.
    AlreadyServing(u16),
    /// Free, or at least not answering as Samong.
    Free(u16),
}

/// Decide where to serve, starting at `first`.
pub fn choose_port(first: u16) -> Result<Where> {
    for port in first..first.saturating_add(PORT_ATTEMPTS) {
        if samong_is_serving(port) {
            return Ok(Where::AlreadyServing(port));
        }
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(Where::Free(port));
        }
    }
    anyhow::bail!(
        "ports {first}-{} are all in use, so there is nowhere to serve from",
        first + PORT_ATTEMPTS - 1
    )
}

/// Where the launcher writes what happened.
///
/// A GUI launcher has nowhere to print. Without this, a first run that fails
/// fails invisibly — and the person it failed for is the one least equipped to
/// go looking. The file is opened for them when something goes wrong.
pub fn log_path() -> Result<PathBuf> {
    Ok(crate::registry::config_dir()?.join("launcher.log"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_vault_goes_where_a_person_will_find_it() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join("Documents")).unwrap();
        assert_eq!(
            default_vault_dir(home.path()),
            home.path().join("Documents").join("Samong")
        );
    }

    /// Not every machine has a Documents folder; creating one is not this
    /// program's business.
    #[test]
    fn without_a_documents_folder_it_falls_back_to_home() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(default_vault_dir(home.path()), home.path().join("Samong"));
    }

    #[test]
    fn the_welcome_notes_link_to_each_other() {
        let vault = tempfile::tempdir().unwrap();
        write_welcome_notes(vault.path()).unwrap();
        let first = std::fs::read_to_string(vault.path().join("Welcome to Samong.md")).unwrap();
        let second = std::fs::read_to_string(vault.path().join("How Samong works.md")).unwrap();
        assert!(first.contains("[[How Samong works]]"));
        assert!(second.contains("[[Welcome to Samong]]"));
    }

    /// The folder may be one the person already used. Arriving there must not
    /// cost them a file.
    #[test]
    fn welcome_notes_never_overwrite_something_that_is_there() {
        let vault = tempfile::tempdir().unwrap();
        let existing = vault.path().join("Welcome to Samong.md");
        std::fs::write(&existing, "mine\n").unwrap();
        write_welcome_notes(vault.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "mine\n");
        assert!(vault.path().join("How Samong works.md").exists());
    }

    #[test]
    fn a_dead_port_is_not_mistaken_for_a_running_samong() {
        // Bound and immediately dropped, so nothing is listening there.
        let port = {
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            listener.local_addr().unwrap().port()
        };
        assert!(!samong_is_serving(port));
    }

    /// The case that matters: something is listening, but it is not us. Opening
    /// a browser at somebody else's app would be worse than any error.
    #[test]
    fn a_listener_that_is_not_samong_is_not_treated_as_samong() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        // Kept accepting for the life of the test, not just once: the first
        // version answered a single connection and then dropped the listener, so
        // by the time `choose_port` looked, the port was free and it "moved past"
        // an obstacle that no longer existed. The daemon thread ends with the
        // process.
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<!doctype html>",
                );
            }
        });
        assert!(!samong_is_serving(port));
        // And the launcher moves past it rather than failing or, worse, opening a
        // browser at whatever that other program is.
        match choose_port(port).unwrap() {
            Where::Free(chosen) => assert_ne!(chosen, port, "the taken port was not reused"),
            Where::AlreadyServing(_) => panic!("that was not a samong server"),
        }
    }
}
