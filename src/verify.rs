//! Is this vault the one its publisher published?
//!
//! # What is actually missing, and what is not
//!
//! Not integrity. An installed vault is a git checkout, and every byte of it is
//! already covered by the commit hash — a Merkle tree that git rechecks on every
//! operation. Writing a `SHA256SUMS` beside the content would restate what git
//! already guarantees, and restate it *weaker*: anyone who can change a note can
//! change the checksum file sitting next to it. A digest with no signature over
//! it is not a security control, it is a second copy of the thing you doubt.
//!
//! What is missing is **authenticity** — not "did this arrive intact" but "is
//! this from the person I bought it from". That is a signature, and git already
//! knows how to make and check one against the reader's own keyring. So this
//! module is a reader over `git`'s answer, not a new mechanism.
//!
//! # Commits, not tags
//!
//! Signing release *tags* is the older convention, and it is the wrong one here:
//! `samong vault update` follows a branch, so a reader takes new commits between
//! tags and a tag signature says nothing about the commit they just pulled. The
//! seller signs commits (`git config commit.gpgsign true`); every update is then
//! attributable on its own.
//!
//! # Trust on first use
//!
//! Verification only means something if the reader knows *which* key to expect,
//! and no registry can tell them: the first time they install, whoever they got
//! the URL from is the authority. So the signer seen at install is pinned, the
//! way SSH pins a host key, and a later change is refused rather than reported.
//! The pin lives in the clone's own `git config` — same judgement as the rest of
//! Phase 28: the checkout is its own provenance, and a record kept anywhere else
//! could only drift away from the thing it describes.

use std::path::Path;

use anyhow::{Context, Result};

use crate::git;
use crate::install::Installation;

/// Git config key holding the pinned signer, inside the installed clone.
const PIN_KEY: &str = "samong.signer";

/// What git thinks of a commit's signature.
///
/// These are `git log --format=%G?` verbatim rather than a simplification,
/// because the distinctions are the useful part: "signed by someone whose key
/// you do not have" and "not signed at all" call for completely different
/// actions from the reader, and collapsing them into `false` would hide that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trust {
    /// Good signature from a key the reader trusts.
    Good,
    /// Good signature, but the reader has never said they trust that key. The
    /// normal state for a key you downloaded rather than met.
    Untrusted,
    /// Good signature from a key that has expired or been revoked.
    Stale(char),
    /// The content does not match the signature.
    Bad,
    /// The reader does not have the key, so nothing could be checked.
    KeyUnavailable,
    /// No signature at all.
    Unsigned,
}

impl Trust {
    fn from_code(code: char) -> Self {
        match code {
            'G' => Trust::Good,
            'U' => Trust::Untrusted,
            'X' | 'Y' | 'R' => Trust::Stale(code),
            'B' => Trust::Bad,
            'E' => Trust::KeyUnavailable,
            _ => Trust::Unsigned,
        }
    }

    /// Was a signature present and cryptographically sound?
    ///
    /// Deliberately true for `Untrusted`: the reader pinned this key at install,
    /// which is the trust decision. Requiring a keyring certification on top
    /// would make the feature useless to everyone who did not already run their
    /// own web of trust.
    pub fn is_sound(&self) -> bool {
        matches!(self, Trust::Good | Trust::Untrusted)
    }

    pub fn describe(&self) -> &'static str {
        match self {
            Trust::Good => "good signature",
            Trust::Untrusted => "good signature from an uncertified key",
            Trust::Stale('X') => "good signature from an expired key",
            Trust::Stale('Y') => "good signature, key expired at signing time",
            Trust::Stale(_) => "good signature from a revoked key",
            Trust::Bad => "BAD signature — the content does not match it",
            Trust::KeyUnavailable => "signed, but you do not have the key to check it",
            Trust::Unsigned => "not signed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Signature {
    pub trust: Trust,
    /// Who git says signed it. Free text chosen by the signer, so useful to show
    /// and useless to compare — [`Signature::key`] is the identity.
    pub signer: String,
    /// Key id or fingerprint. Empty when unsigned.
    pub key: String,
    pub commit: String,
}

impl Signature {
    /// One line naming both the signer and the key, because the readable half is
    /// the half an impersonator gets to choose.
    pub fn describe_signer(&self) -> String {
        match (self.signer.is_empty(), self.key.is_empty()) {
            (true, true) => "unknown".to_string(),
            (true, false) => self.key.clone(),
            (false, true) => self.signer.clone(),
            (false, false) => format!("{} ({})", self.signer, self.key),
        }
    }
}

/// Read the signature on one revision.
pub fn signature_of(repo: &Path, rev: &str) -> Result<Signature> {
    // A unit separator, because a signer's name is free text and may contain
    // anything a tab or a colon could be mistaken for.
    let raw = git::run(
        &["log", "-1", "--format=%G?%x1f%GS%x1f%GK%x1f%H", rev],
        repo,
    )
    .with_context(|| format!("reading the signature on {rev}"))?;
    let mut fields = raw.split('\u{1f}');
    let code = fields
        .next()
        .and_then(|field| field.trim().chars().next())
        .unwrap_or('N');
    Ok(Signature {
        trust: Trust::from_code(code),
        signer: fields.next().unwrap_or_default().trim().to_string(),
        key: fields.next().unwrap_or_default().trim().to_string(),
        commit: fields.next().unwrap_or_default().trim().to_string(),
    })
}

/// The signer this checkout was installed from, if it was signed at all.
pub fn pinned_signer(repo: &Path) -> Option<String> {
    git::optional(&["config", "--local", "--get", PIN_KEY], repo)
}

/// Remember the signer of a freshly installed vault.
///
/// Nothing is pinned for an unsigned vault: pinning "unsigned" would promise a
/// guarantee we cannot keep, since the reader has no way to tell an unsigned
/// vault from an unsigned impostor of it.
pub fn pin_signer(repo: &Path, signature: &Signature) -> Result<bool> {
    if !signature.trust.is_sound() || signature.key.is_empty() {
        return Ok(false);
    }
    git::run(&["config", "--local", PIN_KEY, &signature.key], repo)?;
    Ok(true)
}

/// Files in an installed vault that differ from what was published — including
/// files nobody published.
///
/// Untracked files count. A stray `.md` dropped into an installed vault would be
/// indexed, would appear in search, and would be attributed to the seller; that
/// it was never signed by anyone is exactly the point.
pub fn local_changes(repo: &Path) -> Result<Vec<String>> {
    Ok(git::run(&["status", "--porcelain"], repo)?
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect())
}

/// Everything known about whether one installed vault is what it claims to be.
pub struct Report {
    pub name: String,
    pub signature: Signature,
    /// The signer pinned at install time, when there was one.
    pub pinned: Option<String>,
    pub changes: Vec<String>,
}

impl Report {
    pub fn for_installation(installation: &Installation) -> Result<Self> {
        Ok(Self {
            name: installation.name.clone(),
            signature: signature_of(&installation.path, "HEAD")?,
            pinned: pinned_signer(&installation.path),
            changes: local_changes(&installation.path)?,
        })
    }

    /// The signer changed since install, or dropped away entirely.
    ///
    /// A vault that was signed and now is not is treated exactly like one signed
    /// by a stranger. Otherwise the cheapest attack on this whole mechanism is
    /// to stop signing.
    pub fn signer_changed(&self) -> bool {
        match &self.pinned {
            None => false,
            Some(pinned) => !self.signature.trust.is_sound() || &self.signature.key != pinned,
        }
    }

    /// Something is wrong — not merely unproven.
    pub fn is_wrong(&self) -> bool {
        self.signature.trust == Trust::Bad || self.signer_changed() || !self.changes.is_empty()
    }

    /// The reader can show this vault is the publisher's.
    pub fn is_proven(&self) -> bool {
        self.signature.trust.is_sound() && !self.is_wrong()
    }
}

/// Refuse to move an installed vault onto a commit signed by somebody else.
///
/// Checked *before* the merge, not after: a warning printed once the content is
/// already on disk and already indexed has told the reader about a decision that
/// was made for them.
pub fn check_before_moving_to(repo: &Path, rev: &str, name: &str) -> Result<()> {
    let Some(pinned) = pinned_signer(repo) else {
        return Ok(());
    };
    let signature = signature_of(repo, rev)?;
    if signature.trust.is_sound() && signature.key == pinned {
        return Ok(());
    }
    anyhow::bail!(
        "refusing to update \"{name}\": it was installed signed by {pinned}, and the new \
         commit is {}{}.\n\
         Nothing has been changed on disk. Look at what arrived:\n\
         \n    git -C <the vault> log --show-signature -1 {rev}\n\
         \nIf the publisher really did change keys and you have confirmed that with them, \
         drop the pin and update again:\n\
         \n    git -C <the vault> config --unset {PIN_KEY}",
        signature.trust.describe(),
        if signature.key.is_empty() {
            String::new()
        } else {
            format!(" by {}", signature.describe_signer())
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_status_codes_map_to_what_the_reader_has_to_do() {
        assert_eq!(Trust::from_code('G'), Trust::Good);
        assert_eq!(Trust::from_code('U'), Trust::Untrusted);
        assert_eq!(Trust::from_code('B'), Trust::Bad);
        assert_eq!(Trust::from_code('E'), Trust::KeyUnavailable);
        assert_eq!(Trust::from_code('N'), Trust::Unsigned);
        // An unrecognised code from a future git must not read as "fine".
        assert_eq!(Trust::from_code('?'), Trust::Unsigned);
        assert!(!Trust::from_code('?').is_sound());
    }

    /// The pin is a decision the reader already made; a key they never certified
    /// is the ordinary case and must not read as a failure.
    #[test]
    fn an_uncertified_but_valid_signature_counts_as_sound() {
        assert!(Trust::Untrusted.is_sound());
        assert!(!Trust::KeyUnavailable.is_sound());
        assert!(!Trust::Stale('R').is_sound());
    }

    fn report(pinned: Option<&str>, trust: Trust, key: &str, changes: Vec<String>) -> Report {
        Report {
            name: "handbook".into(),
            signature: Signature {
                trust,
                signer: "Someone".into(),
                key: key.into(),
                commit: "abc123".into(),
            },
            pinned: pinned.map(str::to_string),
            changes,
        }
    }

    #[test]
    fn an_unsigned_vault_is_unproven_but_not_wrong() {
        let report = report(None, Trust::Unsigned, "", vec![]);
        assert!(!report.is_wrong());
        assert!(!report.is_proven());
    }

    /// The cheapest attack on signature pinning is to simply stop signing.
    #[test]
    fn dropping_the_signature_is_treated_as_changing_it() {
        let report = report(Some("KEY1"), Trust::Unsigned, "", vec![]);
        assert!(report.signer_changed());
        assert!(report.is_wrong());
    }

    #[test]
    fn a_different_key_than_the_one_pinned_is_wrong() {
        assert!(report(Some("KEY1"), Trust::Good, "KEY2", vec![]).signer_changed());
        assert!(!report(Some("KEY1"), Trust::Good, "KEY1", vec![]).signer_changed());
        assert!(report(Some("KEY1"), Trust::Good, "KEY1", vec![]).is_proven());
    }

    /// A locally edited reference note is still a copy that no longer matches
    /// what was published, whoever changed it and however innocently.
    #[test]
    fn local_edits_make_a_signed_vault_unproven() {
        let report = report(
            Some("KEY1"),
            Trust::Good,
            "KEY1",
            vec!["M Runbook.md".into()],
        );
        assert!(report.is_wrong());
        assert!(!report.is_proven());
    }
}
