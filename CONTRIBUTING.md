# Contributing

Bug reports and pull requests are welcome. This file exists so you can tell,
before spending your time, whether a change is likely to be merged and what it
has to satisfy.

## The one thing to know first

**Notes are plain Markdown files in a folder the user already has, and nothing
leaves the machine.** Every other decision follows from that. A change that adds
a network call, an account, a database the notes live inside, or a format only
Samong can read is a change to what the project is — open an issue and make the
case before writing it.

## Before opening a pull request

```sh
cd web && npm run build && cd ..   # the UI is embedded at compile time
cargo test --all --locked
cargo clippy --all --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

All four have to pass. `--locked` everywhere: CI judges the dependency set that
ships, not whatever resolved newest today.

If you touched the web UI, `cargo install --path . --force` afterwards or you will
keep testing the old interface — it is baked into the binary.

## What a good change looks like here

- **A test that would have failed before it.** Not a test that exercises the new
  code, one that fails without the fix. Several bugs in this project's history
  were invisible to a full green suite; the notes in `PLAN.md` say which and why.
- **The reasoning, in the code.** Comments here explain *why* a decision was made,
  especially where the obvious approach was rejected. A decision with no reason
  recorded becomes folklore, and the next person cannot tell an intentional
  constraint from an accident.
- **Failures that are visible.** Anything that can fail silently will. `samong
  update` reported success while replacing nothing, through five releases; a
  `cargo publish` shipped a crate that could not compile. Prefer a loud error over
  a quiet fallback.
- **Nothing claimed that is not true.** Semantic search is off by default and
  absent from the published binaries; the binaries are unsigned. Both are said
  plainly in the README and should stay that way.

## Things that will be declined

- Making reference notes writable, or adding a delete tool to the MCP server.
  Notes pulled in from someone else's vault are read-only on purpose: an edit
  would be erased by the next update, and the content is not the reader's to
  change.
- A sync protocol of Samong's own. A vault is a folder; git already moves folders
  between machines, and the project is not going to grow a second answer.
- Tests that touch the real registry at `~/.config/samong`. They fought over
  redb's lock when run in parallel and modified vaults people actually used. Set
  `SAMONG_CONFIG_DIR` to a temporary directory.

## Commit messages

Long-form, explaining the decision rather than the diff — `git log` is the design
record for this project. Existing messages are in Thai; **English is equally
welcome**, and mixed history is fine.

## Licence and the name

Code is Apache-2.0. By opening a pull request you agree your contribution is
licensed the same way.

The name "Samong" and the logo are **not** covered by that licence — see
`site/brand/LICENSE`. You may fork the code freely; please do not ship a fork
under this name, so that users can tell who maintains what they installed.
