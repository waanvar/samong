//! Samong — local-first, Obsidian-compatible knowledge base.
//! Shared library behind the `samong` CLI and `samong-server` API binaries.

pub mod cli;
pub mod graph;
pub mod indexer;
pub mod mcp;
pub mod ops;
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
pub mod watch;
