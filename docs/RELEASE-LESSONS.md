# What went wrong shipping this, and what to check because of it

Written 2026-08-05, after the sequence that took Samong from a private repository to
crates.io, the MCP registry, Homebrew, Scoop and a landing page. Kept because every
item below was a real defect that reached a published artefact or was one step away
from doing so, and because they share a shape.

## The shape

**Green CI never meant the artefact was correct.** Not one of the shipping defects
below turned a CI job red. Each was caught by downloading the published thing and
running it — or was not caught at all until a user-facing surface was inspected by
hand.

Three specific ways a check can be worthless:

1. **A success message printed unconditionally.** `samong update` announced
   "updated to <version>" while every binary had been skipped. That one message hid
   *three further faults* for five releases. It is the most expensive bug class here:
   silent success does not merely fail, it conceals.
2. **A dry run that cannot fail the way the real command fails.** CI ran
   `cargo publish --dry-run --allow-dirty`, so it passed while `cargo publish`
   refused outright.
3. **A test whose other branch never runs.** `embedded_ui_serves_index_or_…` asserted
   404 for the no-UI case and passed on every developer machine for months, because a
   working tree that has ever run `npm run build` always takes the other branch.

And two ways a reading can be worthless:

4. **Reading a schema is not validating against it.** The MCP registry rejected a
   release for a `description` over 100 characters. That limit was in the schema the
   whole time; it had been read for `enum` values only.
5. **A schema is not the authority.** The same registry forbids `registryBaseUrl` on
   mcpb packages *in code*, while the schema permits it on any package. The rule is
   only visible in `internal/validators/registries/mcpb.go`. **When something rejects
   a submission, read its validator source.**

## The defects

### Reached a published release

| | What |
|---|---|
| v0.3.3 | The Windows archive's `samong.exe` was the **GUI launcher**, not the CLI. `cp samong-app.exe Samong.exe` on a case-insensitive filesystem replaced it. Release withdrawn. |
| v0.3.0–v0.3.5 | **`samong update` never worked.** Four faults at once: unconditional success message; wrong in-archive path; `self_update` missing `compression-zip-deflate` so Windows zips could not be decompressed; and `bin_install_path` defaulting to the *running* exe, so a loop over three binaries left `samong.exe` containing `samong-mcp`. |
| v0.3.0–v0.3.4 | **Four download buttons on the site, one destination.** Each was labelled by platform and architecture; all four went to the releases page. A direct link needs the exact filename, and every filename carried the version. |
| v0.3.7 | MCP registry rejected the entry: description 170 characters against a limit of 100. |
| v0.3.8 | MCP registry rejected it again: `registryBaseUrl` forbidden for mcpb. |
| v0.3.0–v0.3.1 | `x86_64-macos` asked for a retired `macos-13` runner. The job did not fail — it sat **queued** for six hours. |
| v0.3.0 | The tag was moved locally and never re-pushed, so the published build predated the website advertising it. |

### Caught one step before publishing

- **The crate would not have compiled.** `cargo package` honours `.gitignore`, so
  `assets/words_th.txt` — `include_str!`d by `src/thai.rs` — was excluded. Found by
  extracting the real `.crate` into a clean directory and installing from it. Since
  crates.io versions are immutable, publishing would have burned the number.
- **The crate would have had no web UI.** Same cause: `web/dist` is gitignored, so
  the package carried an empty `.gitkeep`. The launcher would have opened a browser
  onto a 404 in a program with no console.
- **`include` unanchored pulled in 149 files from `web/node_modules`.** Cargo reads
  those patterns as gitignore globs, where a bare `README.md` matches at any depth.
- **A `[[` in prose swallowed the next real link.** The welcome note written on a
  fresh install *described* wikilinks, and the pattern matched across line breaks,
  drawing a graph node whose label was a paragraph. Found by opening a brand-new
  vault and reading the graph — no test saw it.

### My own checks, wrong before they were right

- `check-publish-tree.sh` v1 used `git status`, which **does not list ignored files
  at all** — it reported zero while cargo counted 55. Its "1 generated file" was an
  empty line being counted: a message claiming more than the check had done.
- v2 asked git for every ignored path and flagged `target/` and `web/node_modules/`,
  which are not in the package. The file list has to come from
  `cargo package --list` — cargo's own view.
- The MCP bundle manifest advertised five tools; the server has six.

### Environment traps, each cost a cycle

- **Local clippy is 1.93, CI is 1.97.** A clean local lint run is not evidence.
- **`bash` from PowerShell is WSL's bash**, not Git Bash — and WSL is broken on this
  machine. Use `C:\Program Files\Git\bin\bash.exe`.
- **PowerShell `-notmatch` against an array filters** instead of returning a boolean.
  A *passing* search looked like a failure until piped through `Out-String`.
- **`winget validate --manifest <dir>` parses every file in the directory** — a
  generator sitting beside the manifests broke it.
- **`brew` is absent on ubuntu runners**, Homebrew **refuses formulae given as a
  path**, and `brew test` does not accept `--formula` though `brew install` does.
- **Heredocs mangle backslashes and non-ASCII.** Several edits silently lost `\`
  continuations and `\n` escapes. Use the file-writing tool for content with escapes.
- **The disk hit zero bytes free**; `target/` alone was 13.1 GB.

## The checks that exist because of the above

| Check | Catches |
|---|---|
| `packaging/verify-stage.sh` | two differently-named binaries that became one file |
| `packaging/check-publish-tree.sh` | uncommitted source going out under `--allow-dirty` |
| CI job `package` | `cargo package --list` missing the UI or the dictionary, plus `publish --dry-run` |
| CI job `winget` | committed manifests drifting from the generator; a real local-manifest install |
| CI job `aur` | `.SRCINFO` out of step with the PKGBUILD; `makepkg`, `namcap`, `pacman -U` |
| tap / bucket CI | `brew`/`scoop` install on real runners; **no `com.apple.quarantine`** on the installed binary |
| `validate-server-json.py` | the whole registry schema, plus nine mcpb rules read out of the registry's Go source |
| `mcp_registry.rs` tests | `server.json` drifting from `Cargo.toml`; the two field limits that actually bit |
| `update.rs` tests | in-archive path vs. what the release workflow builds; the cargo features needed to unpack |

**After any release: `gh run view <id> --json jobs` (a queued job is a failure CI
does not report), open an archive, and run the published binary.** The last one is
what caught the updater; no test could.

## What to build next

Ordered by what the work so far actually revealed, not by appeal.

### 1. A read-only reader for phones — `samong publish` → static site

The single biggest gap. A vault someone buys or is given is read on a phone, and
there is no way to read one at all today. The web UI, `rust-embed` and the landing
page pipeline already exist; this is mostly assembly. It is also what makes the
`pack` / `install` / `verify` work reach anyone who is not a developer.

### 2. A Sync button that wraps git

Non-developers cannot `git pull`, and vault updates are git operations.
**Constraint: never grow a sync protocol of our own** — a vault is a folder and git
already moves folders. This is UI over `commit`/`pull`/`push`, nothing more.

### 3. Make semantic search reachable

It is off by default *and absent from every published binary*, so the differentiator
cannot be experienced by anyone who downloads Samong. A second `samong-ai` binary in
the same release keeps the "one binary, nothing to fetch" promise for the default
while making the feature real. The 465 MB model download stays opt-in at runtime.

### 4. Finish AUR, and the packaging debt

- `.SRCINFO` must be committed — only `makepkg --printsrcinfo` can produce it, so CI
  prints it and it gets committed from there.
- Icons: `.ico` and `.icns` need a rasteriser in CI and, for Windows, a build script
  to embed the resource. A bundle with no icon opens but looks unfinished.
- `CODE_OF_CONDUCT.md`, issue and pull-request templates — GitHub reports 71% health.
- No `aarch64-linux` or Windows ARM64 archive exists; brew, scoop and AUR each
  document the hole rather than pretend.

### 5. A thin `samong-mcp` crate

Would make `registryType: "cargo"` usable in the MCP registry. Blocked today because
a cargo entry has clients invoke the binary named after the crate, and this crate is
`samong`, whose `samong` binary is the CLI. Two crates sharing one version number is
its own release-time trap, so this is worth doing deliberately or not at all.

### 6. Ranking and graph rough edges

- **RRF has no similarity floor**: an unremarkable semantic hit can reach position 2.
- **Graph labels collide** in dense areas.
- `samong pack` does not copy attachments — images and PDFs a note links to are not
  `.md` and are left behind. Documented, not fixed.

### 7. Code signing, reassessed

It was "the largest remaining barrier". It is now smaller: Homebrew and Scoop both
avoid the OS prompt entirely, and `cargo install` never had one. Apple Developer is
$99/year; Azure Trusted Signing is $9.99/month but restricted to organisations in the
US and Canada trading for three years, which likely excludes a solo developer here.
**Worth revisiting only when enough people install via the double-click path to know
it matters.**
