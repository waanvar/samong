mod cli;
mod graph;
mod search;
mod vault;

fn main() -> anyhow::Result<()> {
    cli::run()
}
