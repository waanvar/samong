use clap::Parser;

#[derive(Parser)]
#[command(
    name = "banyan-server",
    version,
    about = "Local REST/WebSocket API for banyan vaults (binds 127.0.0.1 only)"
)]
struct Args {
    /// Port to listen on (always bound to 127.0.0.1)
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    banyan::server::run(args.port).await
}
