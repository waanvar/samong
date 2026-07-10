use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "banyan-server",
    version,
    about = "Local REST/WebSocket API + web UI for banyan vaults (binds 127.0.0.1 only)"
)]
struct Args {
    /// Port to listen on (always bound to 127.0.0.1)
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Directory of the built web UI (vite build output)
    #[arg(long, default_value = "web/dist")]
    ui: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    banyan::server::run(args.port, Some(args.ui)).await
}
