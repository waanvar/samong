//! Running `git`.
//!
//! # Why the `git` binary rather than a Rust git library
//!
//! Because of authentication, and then because of signatures. A vault someone
//! sells lives in a private repository, and reaching it means SSH agents,
//! credential helpers, hardware keys, SSO device flows and 2FA tokens —
//! everything the user has *already* configured for `git`. Verifying who
//! published it means GPG keyrings, SSH allowed-signers files, and x509
//! backends — everything the user has already configured for `git`. Embedding a
//! library means reimplementing both badly, and the first person to hit a setup
//! we did not anticipate simply cannot get, or cannot check, what they paid for.
//!
//! The cost is a dependency on a program almost every user of this already has,
//! and a clear message when they do not.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};

/// Run git, failing if it does.
pub fn run(args: &[&str], cwd: &Path) -> Result<String> {
    let output = raw(args, cwd)?;
    if !output.status.success() {
        bail!(
            "git {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run git where a non-zero exit is an answer, not a failure.
///
/// `git config --get` exits 1 for "not set" and `rev-parse @{u}` exits 128 for
/// "no upstream configured". Both are ordinary states of a repository, and
/// treating them as errors would turn "you have not pinned a key yet" into a
/// crash.
pub fn optional(args: &[&str], cwd: &Path) -> Option<String> {
    let output = raw(args, cwd).ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn raw(args: &[&str], cwd: &Path) -> Result<std::process::Output> {
    Command::new("git")
        .args(args)
        .current_dir(cwd)
        // Nothing here may sit waiting for a passphrase or a password prompt on
        // a terminal the caller may not be watching.
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|error| {
            anyhow::anyhow!(
                "could not run `git`: {error}\n\
                 Installing and verifying a vault uses git, so it has to be on PATH. \
                 Install it from https://git-scm.com and try again."
            )
        })
}
