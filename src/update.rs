//! Self-update from GitHub releases: check for a newer version and replace the
//! installed binaries in place. Backs the `samong update` command and the
//! best-effort "new version available" notice printed by `samong-server start`.

use anyhow::{Context, Result};
use self_update::backends::github;

const REPO_OWNER: &str = "waanvar";
const REPO_NAME: &str = "samong";

/// The three binaries this project ships; `samong update` refreshes all of them.
const BINARIES: [&str; 3] = ["samong", "samong-server", "samong-mcp"];

/// This build's version (from Cargo.toml at compile time).
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The release-asset target string, matching the naming in
/// `.github/workflows/release.yml` (e.g. `x86_64-windows`). Returns None on
/// platforms we don't publish builds for.
fn asset_target() -> Option<&'static str> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => Some("x86_64-linux"),
        ("x86_64", "windows") => Some("x86_64-windows"),
        ("aarch64", "macos") => Some("aarch64-macos"),
        ("x86_64", "macos") => Some("x86_64-macos"),
        _ => None,
    }
}

/// The latest published version on GitHub, or None if there are no releases yet.
/// Network errors propagate so callers can decide whether to surface or swallow.
pub fn latest_version() -> Result<Option<String>> {
    let releases = github::ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()?
        .fetch()?;
    Ok(releases.into_iter().next().map(|r| r.version))
}

/// Is `latest` a strictly newer semver than `current`?
pub fn is_newer(current: &str, latest: &str) -> bool {
    self_update::version::bump_is_greater(current, latest).unwrap_or(false)
}

/// Best-effort check used at server startup: prints a one-line notice if a
/// newer version exists. Never fails the caller — offline or no-release is fine.
pub fn notify_if_outdated() {
    let Ok(Some(latest)) = latest_version() else {
        return;
    };
    if is_newer(current_version(), &latest) {
        println!(
            "→ samong {latest} is available (you have {}). Run `samong update` to upgrade.",
            current_version()
        );
    }
}

/// `samong update`: report status, and unless `check_only`, download the latest
/// release and replace every installed binary in place.
pub fn run(check_only: bool) -> Result<()> {
    let target = asset_target().context(
        "no prebuilt release for this platform — update by rebuilding from source instead",
    )?;

    // A failed check (offline, private repo, no releases) shouldn't error out
    // the command — report it plainly and stop.
    let latest = match latest_version() {
        Ok(Some(v)) => v,
        Ok(None) => {
            println!("no releases published yet — nothing to update to");
            return Ok(());
        }
        Err(e) => {
            println!("couldn't check for updates: {e}");
            println!("(the GitHub repo must be public and have a published release)");
            return Ok(());
        }
    };
    let current = current_version();

    if !is_newer(current, &latest) {
        println!("already up to date (samong {current})");
        return Ok(());
    }
    println!("update available: {current} → {latest}");
    if check_only {
        println!("run `samong update` (without --check) to install it");
        return Ok(());
    }

    // Replace each binary. self_update handles the platform specifics,
    // including the Windows "rename the running exe" dance for the current one.
    for bin in BINARIES {
        print!("updating {bin} … ");
        let status = github::Update::configure()
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .bin_name(bin)
            .target(target)
            .current_version(current)
            .no_confirm(true)
            .show_download_progress(false)
            .build()
            .and_then(|u| u.update());
        match status {
            Ok(s) => println!("done ({})", s.version()),
            // A locked binary (e.g. samong-server still running) shouldn't abort
            // the others — report and continue.
            Err(e) => println!("skipped: {e}"),
        }
    }
    println!("updated to {latest}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_newer_compares_semver() {
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.1.0", "1.0.0"));
        assert!(!is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
    }

    #[test]
    fn asset_target_matches_release_naming() {
        // Whatever platform the test runs on, if we publish for it the string
        // must be one the release workflow actually produces.
        if let Some(t) = asset_target() {
            assert!([
                "x86_64-linux",
                "x86_64-windows",
                "aarch64-macos",
                "x86_64-macos",
            ]
            .contains(&t));
        }
    }

    #[test]
    fn current_version_is_set() {
        assert!(!current_version().is_empty());
    }
}
