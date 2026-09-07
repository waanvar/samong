# Samong — instructions for Claude Code

Read `CONTRIBUTING.md` first; it is the contract for what gets merged. This file
only adds what an agent cannot infer from the tree.

## Before you finish a change

```sh
cd web && npm run build && cd ..   # the UI is embedded at compile time
cargo test --all --locked
cargo clippy --all --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

All four must pass before you open a pull request. `--locked` everywhere, on
purpose: CI judges the dependency set that ships, not whatever resolved newest
today. Toolchain is Rust 1.88 (`rust-version` in `Cargo.toml`) and Node 22.

The `semantic` feature is off by default and no release binary carries it, so a
plain `cargo test` never compiles it. If you touched anything under it:

```sh
cargo test --features semantic --locked
cargo clippy --features semantic --all-targets --locked -- -D warnings
```

Do not download the embedding model in a sandbox — it is 465 MB and CI
deliberately stops short of it.

## Traps this project has actually fallen into

- **The web UI is baked into the binary.** Editing `web/` and rebuilding only the
  crate keeps serving the old interface. `npm run build` first, then
  `cargo install --path . --force` if you are testing the binary by hand.
- **Never touch the real registry at `~/.config/samong`.** Tests fought over
  redb's lock in parallel and modified vaults people were using. Point
  `SAMONG_CONFIG_DIR` at a temporary directory.
- **Do not assert on wall-clock time.** Assert what the feature promises. Timing
  assertions turned CI red for three pushes and said nothing about the bug.
- **Case-insensitive filesystems.** `Samong.exe` once overwrote `samong.exe`.
  Anything that builds a path or filename across OSes needs a test that says so.
- **A command that reports success while doing nothing is the defect.** `samong
  update` did exactly that through five releases. Prefer a loud error over a
  quiet fallback, and add the test that would have caught it — not a test that
  merely exercises the new code.
- **New dependency → update `THIRD-PARTY.md`** in the same commit.

## Things that will be declined, so do not propose them

- A delete tool on the MCP server, or writable reference notes.
- A sync protocol of Samong's own — a vault is a folder, and git already moves
  folders between machines.
- Anything that adds a network call, an account, or a format only Samong reads.

## Voice

README, site, CLI and release notes are understated and precise, and state
limitations in the same paragraph as the claim. No hype adjectives, no benchmark
numbers that were not measured, nothing claimed that is not released. The
binaries are unsigned and semantic search is off by default; both are said
plainly and stay that way.

Commit messages are long-form and explain the decision, not the diff. Thai or
English, both fine.
