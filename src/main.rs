mod cli;
mod graph;
mod indexer;
mod registry;
mod search;
mod vault;
mod watch;

fn main() -> anyhow::Result<()> {
    cli::run()
}
