//! Samong, opened by double-clicking it.
//!
//! Takes no arguments and asks no questions: it finds or creates a vault, finds
//! a port, starts the server, and opens the browser. See [`samong::app`] for
//! why each of those is a decision rather than a default.
//!
//! # No console window
//!
//! On Windows a console binary launched from Explorer opens a black window that
//! stays for as long as the program runs, and closing it kills the program. That
//! window is the single clearest signal that something is "not really an app",
//! so this binary is built for the windows subsystem instead — at the cost of
//! having nowhere at all to print, which is what the log file below is for.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::io::Write;

use samong::app::{self, Where};

fn main() {
    let mut log = Log::open();
    if let Err(error) = run(&mut log) {
        // The only channel a windowless process has to a person: write it down,
        // then open it in whatever they read text files with. Silence here would
        // land on exactly the user least able to go looking for a reason.
        log.write(&format!("\nfailed: {error:#}\n"));
        if let Some(path) = &log.path {
            let _ = open::that(path);
        }
        std::process::exit(1);
    }
}

fn run(log: &mut Log) -> anyhow::Result<()> {
    log.write(&format!("samong-app {}\n", env!("CARGO_PKG_VERSION")));

    // Two escape hatches, both environment variables because a double-clicked
    // program has no arguments to pass. `SAMONG_PORT` is for a machine where the
    // usual range is occupied by something that stays; `SAMONG_NO_OPEN` is for
    // running it without a browser at all — a headless box, or a test.
    let first = std::env::var("SAMONG_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(app::DEFAULT_PORT);
    let open_browser = std::env::var_os("SAMONG_NO_OPEN").is_none();

    let port = match app::choose_port(first)? {
        // Already open. Bring the person to the window that exists instead of
        // starting a second copy that fights the first for the same vault.
        Where::AlreadyServing(port) => {
            log.write(&format!("already serving on {port}; opening the browser\n"));
            if open_browser {
                open::that(format!("http://127.0.0.1:{port}"))?;
            }
            return Ok(());
        }
        Where::Free(port) => port,
    };

    #[allow(deprecated)] // un-deprecated in Rust 1.85; kept for older lints
    let home = std::env::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    if let Some(created) = app::ensure_a_vault(&home)? {
        log.write(&format!(
            "first run: created vault \"{}\" at {}\n",
            created.name,
            created.path.display()
        ));
    }

    // A launcher whose whole purpose is the browser window must not open one onto
    // a server with no interface to show. That would be a blank page in a program
    // with no console — the exact failure this binary exists to avoid. It happens
    // when the crate is built without `web/dist` populated.
    let open_browser = if open_browser && !samong::server::has_embedded_ui() {
        log.write(
            "this build has no web UI embedded, so the browser was not opened.\n\
             The command line still works. A release build from https://samong.dev \
             ships the interface inside the binary.\n",
        );
        if let Some(path) = &log.path {
            let _ = open::that(path);
        }
        false
    } else {
        open_browser
    };

    log.write(&format!("serving on {port}\n"));
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(samong::server::run(port, None, open_browser))
}

/// Append-only record of one launch.
///
/// Truncated per launch on purpose: the useful question is always "what happened
/// when I just tried to open it", and a file that grows forever is a file nobody
/// can read the top of.
struct Log {
    path: Option<std::path::PathBuf>,
    file: Option<std::fs::File>,
}

impl Log {
    fn open() -> Self {
        let path = app::log_path().ok();
        let file = path.as_ref().and_then(|path| {
            path.parent().map(std::fs::create_dir_all);
            std::fs::File::create(path).ok()
        });
        Self { path, file }
    }

    fn write(&mut self, line: &str) {
        if let Some(file) = &mut self.file {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}
