//! The minimum supported Rust version, kept honest across the files that state it.
//!
//! `rust-version` in `Cargo.toml` is the one Cargo enforces, and CI's `msrv` job
//! reads that field rather than a number of its own — so a bump there is followed
//! automatically by the compiler the job installs. The prose is not so lucky:
//! `README.md` promises a version to anyone deciding whether they can build this,
//! and `CLAUDE.md` states one to every agent that works on the repo. Nothing binds
//! either to `Cargo.toml`.
//!
//! That is the same shape of defect the `msrv` job was added to close, one file
//! further out: raise the floor, and a fully green CI keeps telling readers the
//! old number. This is the check that fails when they disagree.

use std::fs;

fn repo_file(name: &str) -> String {
    let path = format!("{}/{name}", env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
}

/// `(major, minor, patch)`, so that "1.88" and "1.88.0" are the same version
/// rather than a spurious failure.
fn parts(version: &str) -> (u32, u32, u32) {
    let mut it = version.split('.').map(|p| p.parse().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

fn declared_msrv() -> String {
    repo_file("Cargo.toml")
        .lines()
        .find_map(|l| l.strip_prefix("rust-version = \""))
        .and_then(|v| v.split('"').next())
        .expect("Cargo.toml declares rust-version")
        .to_string()
}

/// Every "Rust <x.y>" in `text`, as written.
///
/// A dot is required, which is what keeps `Rust 2021` — the edition, not a
/// version — from being read as a claim about the compiler.
fn rust_versions_mentioned(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = text.as_bytes();
    for (i, _) in text.match_indices("Rust ") {
        let rest = &text[i + "Rust ".len()..];
        let taken: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let taken = taken.trim_end_matches('.').to_string();
        if taken.contains('.') && bytes.get(i + "Rust ".len()).is_some_and(u8::is_ascii_digit) {
            found.push(taken);
        }
    }
    found
}

/// The number a reader acts on. Someone on an older toolchain reads this line to
/// decide whether `cargo install samong` will work for them, and a stale one
/// sends them into a wall of compiler errors instead of one sentence.
#[test]
fn readme_states_the_msrv_that_cargo_enforces() {
    let msrv = declared_msrv();
    let mentions = rust_versions_mentioned(&repo_file("README.md"));
    assert!(
        !mentions.is_empty(),
        "README.md no longer names a Rust version; it should still tell readers \
         they need Rust {msrv} or newer"
    );
    for m in &mentions {
        assert_eq!(
            parts(m),
            parts(&msrv),
            "README.md says Rust {m}, Cargo.toml says {msrv}"
        );
    }
}

/// Agents are told the toolchain here and do not read Cargo.toml to check it, so
/// a stale number sends them to reproduce a build on a compiler this crate no
/// longer supports.
#[test]
fn the_agent_instructions_state_the_msrv_that_cargo_enforces() {
    let msrv = declared_msrv();
    for m in rust_versions_mentioned(&repo_file("CLAUDE.md")) {
        assert_eq!(
            parts(&m),
            parts(&msrv),
            "CLAUDE.md says Rust {m}, Cargo.toml says {msrv}"
        );
    }
}
