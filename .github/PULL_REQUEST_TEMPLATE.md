<!--
Short PRs do not need every heading. Delete what does not apply — an empty
heading is worse than no heading, and a one-line typo fix needs one line.
-->

## What this changes, and why

<!-- The reasoning, not the diff. The diff is right there. If the obvious
     approach was rejected, say which and why — that is the part which cannot be
     recovered from the code later. -->

## The check that would have failed before this

<!-- CONTRIBUTING.md asks for a test that fails *without* the fix, not one that
     exercises the new code. Name it, or say why there isn't one.

     This project has shipped several defects that a fully green suite could not
     see: an updater that printed success while replacing nothing, a test whose
     other branch never ran on a developer machine, a crate that would not
     compile. docs/RELEASE-LESSONS.md has the list. So: what would this have
     looked like if it were wrong, and what now notices? -->

## Before requesting review

- [ ] `cargo test --all --locked`
- [ ] `cargo clippy --all --all-targets --locked -- -D warnings`
- [ ] `cargo fmt --all -- --check`
- [ ] `cd web && npm run build` — **run this first if you touched the web UI**, and
      then `cargo install --path . --force`, or you are testing the interface
      baked into the old binary rather than yours

<!-- Local clippy can be older than CI's. A clean run here is evidence, not
     proof; CI is the one that decides. -->

## Anything a reviewer should be told rather than discover

<!-- A behaviour change for existing users, a new dependency and why it earns its
     place, something left deliberately unfinished, a claim in the README that is
     now less true. Say it here rather than letting it be found in review. -->
