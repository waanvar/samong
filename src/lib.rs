//! Samong — local-first, Obsidian-compatible knowledge base.
//! Shared library behind the `samong` CLI and `samong-server` API binaries.

/// The desktop launcher: what happens when someone double-clicks Samong.
pub mod app;
pub mod cli;
pub mod git;
pub mod graph;
pub mod indexer;
pub mod install;
pub mod mcp;
pub mod ops;
pub mod provenance;
pub mod registry;
pub mod scope;
pub mod search;
/// Local embeddings. Optional: see the `semantic` feature in Cargo.toml for why
/// meaning-based search is not something everyone should have to download.
#[cfg(feature = "semantic")]
pub mod semantic;
pub mod server;
pub mod thai;
pub mod update;
pub mod vault;
#[cfg(feature = "semantic")]
pub mod vectors;
pub mod verify;
pub mod watch;
