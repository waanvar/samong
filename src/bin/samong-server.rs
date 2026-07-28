use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "samong-server",
    version,
    about = "Local REST/WebSocket API + web UI for samong vaults (binds 127.0.0.1 only)"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    // Options accepted without a subcommand, so `samong-server` and
    // `samong-server --port 8080` keep working alongside the new
    // `samong-server start` form.
    #[command(flatten)]
    opts: ServerOpts,
}

#[derive(Subcommand)]
enum Command {
    /// Start the server (this is also the default action)
    Start {
        #[command(flatten)]
        opts: ServerOpts,
    },
}

#[derive(Args)]
struct ServerOpts {
    /// Port to listen on (always bound to 127.0.0.1)
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Serve the web UI from this directory instead of the one built into
    /// the binary (useful during UI development)
    #[arg(long)]
    ui: Option<PathBuf>,

    /// Don't open the browser automatically on start
    #[arg(long)]
    no_open: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let opts = match cli.command {
        Some(Command::Start { opts }) => opts,
        None => cli.opts,
    };
    samong::server::run(opts.port, opts.ui, !opts.no_open).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> ServerOpts {
        let cli = Cli::try_parse_from(args).expect("args should parse");
        match cli.command {
            Some(Command::Start { opts }) => opts,
            None => cli.opts,
        }
    }

    #[test]
    fn bare_invocation_uses_defaults() {
        let o = parse(&["samong-server"]);
        assert_eq!(o.port, 3000);
        assert!(o.ui.is_none());
        assert!(!o.no_open);
    }

    #[test]
    fn legacy_flags_without_subcommand_still_work() {
        let o = parse(&["samong-server", "--port", "8080", "--no-open"]);
        assert_eq!(o.port, 8080);
        assert!(o.no_open);
    }

    #[test]
    fn start_subcommand_with_options() {
        let o = parse(&["samong-server", "start", "--port", "9000"]);
        assert_eq!(o.port, 9000);
    }
}
