//! Self-update from GitHub releases: check for a newer version and replace the
//! installed binaries in place. Backs the `samong update` command and the
//! best-effort "new version available" notice printed by `samong-server start`.

use anyhow::{Context, Result};
use self_update::backends::github;

const REPO_OWNER: &str = "waanvar";
const REPO_NAME: &str = "samong";

/// The three binaries this project ships; `samong update` refreshes all of them.
const BINARIES: [&str; 3] = ["samong", "samong-server", "samong-mcp"];

/// Where each binary sits inside a release archive.
///
/// Every archive unpacks into one directory named after the release —
/// `samong-v0.3.5-x86_64-windows/samong.exe` — and without this, `self_update`
/// looks for the binary at the archive root, finds nothing, and reports
/// "specified file not found in archive".
///
/// That is not a hypothetical: `samong update` had never worked, in any release.
/// It printed "updated to <version>" while every binary was skipped, so the
/// failure looked like a success and nothing ever contradicted it. The templating
/// is `self_update`'s own — `{{ version }}` is the release version without the
/// leading `v`, which is why the `v` is written out here.
const BIN_PATH_IN_ARCHIVE: &str = "samong-v{{ version }}-{{ target }}/{{ bin }}";

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

    // Where the binaries live: beside this one. `self_update` defaults
    // `bin_install_path` to the *running* executable, so a loop over three names
    // without this overwrites the running binary three times and leaves whichever
    // came last — `samong.exe` ended up being `samong-mcp`. The install path has
    // to name the file being replaced, not the file doing the replacing.
    let install_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        .context("cannot tell where the installed binaries are")?;

    let mut replaced = 0usize;
    let mut skipped = Vec::new();
    for bin in BINARIES {
        print!("updating {bin} … ");
        let install_path = install_dir.join(format!("{bin}{}", std::env::consts::EXE_SUFFIX));
        let status = github::Update::configure()
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .bin_name(bin)
            .bin_install_path(&install_path)
            .bin_path_in_archive(BIN_PATH_IN_ARCHIVE)
            // Pin to the versioned archive. Each release also publishes an
            // unversioned copy so the website can link to a file directly, and
            // without this the choice between them comes down to which name sorts
            // first — a detail that must not decide what gets installed.
            .identifier(&format!("v{latest}"))
            .target(target)
            .current_version(current)
            .no_confirm(true)
            .show_download_progress(false)
            .build()
            .and_then(|u| u.update());
        match status {
            Ok(s) => {
                replaced += 1;
                println!("done ({})", s.version());
            }
            // A locked binary (e.g. samong-server still running) shouldn't abort
            // the others — report and continue.
            Err(e) => {
                println!("skipped: {e}");
                skipped.push(bin);
            }
        }
    }

    // Say what actually happened. Claiming success while nothing was replaced is
    // how a broken updater survives several releases unnoticed.
    if replaced == 0 {
        anyhow::bail!(
            "nothing was updated — every binary failed above.
             Download {latest} by hand instead:              https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/latest"
        );
    }
    if skipped.is_empty() {
        println!("updated to {latest}");
    } else {
        println!(
            "updated {replaced} of {} to {latest} — still on the old version: {}",
            BINARIES.len(),
            skipped.join(", ")
        );
    }
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

    /// The path inside the archive is agreed with the release workflow and
    /// nothing at runtime can check it — a wrong value fails only on a real
    /// download, from a released binary, on a user's machine. That is how this
    /// went unnoticed through five releases.
    ///
    /// So the agreement is asserted against the workflow file itself: it stages
    /// into `samong-${TAG}-<target>/`, tags are `vX.Y.Z`, and `self_update`
    /// substitutes `{{ version }}` without the leading `v`.
    #[test]
    fn the_archive_path_matches_what_the_release_workflow_builds() {
        // Read at run time rather than `include_str!`: the workflow is a
        // repository file and has no business inside a published crate, and
        // embedding it would make the package fail to compile the moment CI
        // files are excluded. Absent means "running from a packaged crate",
        // where there is nothing to check.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/.github/workflows/release.yml");
        match std::fs::read_to_string(path) {
            Ok(workflow) => assert!(
                workflow.contains(r#"name="samong-${TAG}-${{ matrix.target }}""#),
                "the workflow no longer names the staged directory the way                  BIN_PATH_IN_ARCHIVE expects"
            ),
            Err(_) => eprintln!("skipped: no workflow file here (packaged crate)"),
        }
        assert_eq!(
            BIN_PATH_IN_ARCHIVE,
            "samong-v{{ version }}-{{ target }}/{{ bin }}"
        );
    }

    /// Zip entries are deflated, and `archive-zip` on its own only decompresses
    /// stored ones — the omission that made every Windows self-update fail with
    /// "Compression method not supported".
    #[test]
    fn zip_deflate_support_is_compiled_in() {
        let manifest = include_str!("../Cargo.toml");
        let line = manifest
            .lines()
            .find(|l| l.starts_with("self_update"))
            .expect("self_update is a dependency");
        for feature in [
            "archive-zip",
            "compression-zip-deflate",
            "compression-flate2",
        ] {
            assert!(
                line.contains(feature),
                "self_update needs the {feature} feature to unpack a release"
            );
        }
    }
}
