# Banyan 🌳

[![CI](https://github.com/waanvar/banyan/actions/workflows/ci.yml/badge.svg)](https://github.com/waanvar/banyan/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)

**A local-first second brain in Rust — with real Thai word-segmented full-text search**

Your notes are plain Markdown files, fully compatible with
[Obsidian](https://obsidian.md) (`[[wikilink]]` / `[[wikilink|alias]]`).
Banyan adds a fast link graph, full-text search that **actually segments Thai**,
cross-vault links, a local API, and a web UI — while the `.md` files remain
the single source of truth.

*[เวอร์ชันภาษาไทย →](README.md)*

![Banyan editor](docs/editor-dark.png)

## Why Banyan

- 🇹🇭 **Finds Thai words mid-sentence** — Thai has no spaces between words, so
  ordinary search engines see a whole sentence as one token. Banyan segments
  Thai with the newmm dictionary
  ([nlpo3](https://github.com/PyThaiNLP/nlpo3)); searching
  "ตลาดหลักทรัพย์" matches "ตลาดหลักทรัพย์แห่งประเทศไทยเปิดทำการ" with
  highlighted snippets. Obsidian cannot do this.
- 📁 **Your files, your machine** — notes are plain Markdown with zero lock-in.
  Every index lives in `<vault>/.brain/` and can always be rebuilt with
  `banyan reindex`.
- 🔗 **Multi-vault** — link across projects with `[[vault-name/note-title]]`;
  cross-vault backlinks are fully tracked.
- ⚡ **Fast** — link graph in [redb](https://github.com/cberner/redb),
  search by [tantivy](https://github.com/quickwit-oss/tantivy), and
  incremental reindexing that only touches changed files.
- 🤖 **A brain for AI agents** — `banyan-mcp` plugs into Claude Code /
  Claude Desktop over MCP so agents can search, read, and save knowledge
  themselves ([setup guide](docs/AI-AGENT.md)).

## Install

### Requirements

Banyan currently installs by **building from source** (not public yet, no
prebuilt release binaries), so you need the developer toolchain:

- [**Rust**](https://rustup.rs) (stable) — **required**, builds the program
- [**Node.js**](https://nodejs.org) 20+ — **required if you want the web UI**,
  which is embedded into the binary at build time. Without Node you still get
  the CLI + API (`banyan-server` serves API only)

> **Once it goes public**: prebuilt release binaries will be available — end
> users just extract and run `banyan-server start`, **no Rust or Node needed**
> (release.yml builds binaries for all three OSes automatically on a version tag).

### Build from source

```sh
git clone https://github.com/waanvar/banyan.git
cd banyan
cd web && npm install && npm run build   # build the web UI first (it gets embedded)
cd .. && cargo install --path .          # installs banyan / banyan-server / banyan-mcp
```

> **Order matters**: build the web UI before `cargo build`/`cargo install` —
> `banyan-server` **embeds the web UI into the binary**, so it ships as a single
> file with no UI folder alongside it. (To build without installing, use
> `cargo build --release`; binaries land in `target/release/`.)

Update to the latest version later with `banyan update` (see *Updating* below).

## Quickstart

```sh
mkdir my-vault && cd my-vault
banyan new "My First Note"         # create + index
banyan vault add my-vault .        # register in ~/.config/banyan
banyan-server start               # opens http://127.0.0.1:3000 in your browser
```

`banyan-server start` serves the embedded web UI and opens your browser — no UI
files needed alongside it. Change the port with `--port 8080`, skip the browser
with `--no-open` (the old `banyan-server --port 8080` form still works).

![Graph view](docs/graph-dark.png)

## CLI commands

| Command | What it does |
|---|---|
| `banyan new <title>` | Create a note + index it |
| `banyan edit <title>` | Open in `$EDITOR`, reindex on close |
| `banyan rename <old> <new>` | Rename + rewrite every `[[wikilink]]` pointing at it |
| `banyan delete <title>` | Delete + warn about dangling backlinks |
| `banyan links <title> [--all-vaults]` | Forward links + backlinks (incl. cross-vault) |
| `banyan orphans` / `banyan broken` | Unlinked notes / links to missing notes |
| `banyan search <q> [--vault <name>\|--all-vaults]` | Full-text search (Thai/English) |
| `banyan graph [--all-vaults]` | Link-graph edges |
| `banyan list` | List every note |
| `banyan reindex [--full]` | Sync the index (changed files only / everything) |
| `banyan watch` | Watch the vault, keep the index fresh |
| `banyan vault add/list/remove` | Manage the central registry |
| `banyan doctor` | Report what counts as a note, what was skipped, and any ambiguous titles |
| `banyan update [--check]` | Update to the latest GitHub release (--check only reports) |

### What counts as a note (vault scope)

One rule: **a note is a `.md` file you would commit.** Point `banyan vault add`
straight at a project root — no configuration needed. Banyan will:

- respect `.gitignore`, so `node_modules/`, `dist/` and `target/` never get indexed
- always skip dependency directories even when they are not gitignored
  (`node_modules`, `vendor`, `site-packages`, `__pycache__`, `Pods`, `bower_components`)
- skip every dot-directory (`.git`, `.obsidian`, `.brain`)

`banyan doctor` shows what that adds up to:

```sh
banyan doctor
# vault: /home/me/myproject
# gitignore: respected
# 4 note(s) in scope
# skipped 90 .md file(s) not tracked as notes (web 90)
```

To adjust it, add `banyan.toml` at the vault root — **commit it**, so every
machine and any central server reads the same rules. Every field is optional:

```toml
[vault]
name = "myproject"        # the name used in [[myproject/note]] links

[scope]
notes_dir = "docs"        # only scan this subtree (default ".")
exclude = ["archive/**"]  # extra rules, gitignore syntax
follow_gitignore = true   # turn off to index gitignored files too
max_depth = 0             # 0 = unlimited
```

If your repo gitignores its own notes, `.banyanignore` brings them back. Same
syntax as gitignore, negation included:

```
!notes/
drafts/
```

> Deliberately ignored: the global gitignore (`~/.config/git/ignore`),
> `.git/info/exclude`, and `.gitignore` files above the vault. Those are
> per-machine, and honoring them would make one repo index differently on two
> laptops.

### Updating

`banyan update` downloads the latest GitHub release and replaces all three
binaries (banyan / banyan-server / banyan-mcp) in place — including the embedded
web UI. `banyan update --check` reports whether a newer version exists without
installing, and `banyan-server start` prints a one-line notice when an update is
available (best-effort; never blocks, never fails offline).

> A published GitHub release is required first
> (`git tag v0.1.0 && git push origin v0.1.0` triggers the workflow that builds
> binaries for all three OSes) before `banyan update` can find anything.

## Web UI

An original design — banyan-tree identity, not an Obsidian clone — with
first-class Thai typography (IBM Plex Sans Thai, bundled, works offline). The
whole UI is **embedded into the `banyan-server` binary** (rust-embed) — ships
as one file, runs instantly.

- Three-pane layout: notes / editor (write–split–read) / backlinks + outline
- Type `[[` for note autocomplete across vaults; click wikilinks to follow
  (missing notes are created on the spot)
- `Ctrl+K` command palette: open, full-text search, create
- Force-directed graph view (d3): click a node to open it, per-vault colors
  in combined mode
- Dark/light themes, autosave, real-time over WebSocket — edit a file in
  Obsidian or any editor and the page updates itself

UI development: `cd web && npm run dev` (Vite proxies to banyan-server on
port 3000).

## API (banyan-server)

Binds to `127.0.0.1` only (local-first, no auth).

| Endpoint | Purpose |
|---|---|
| `GET /api/vaults` | Registered vaults |
| `GET /api/vaults/{vault}/notes` | Note titles in a vault |
| `GET/PUT/DELETE /api/notes/{vault}/{title}` | Read / write / delete markdown |
| `GET /api/notes/{vault}/{title}/links` | Forward + backlinks + cross-vault |
| `GET /api/search?q=&vault=` | Search (omit `vault` for all vaults) — results include the file `path` |
| `GET /api/graph?vault=` | Nodes + edges as JSON |
| `WS /ws` | Events when .md files change |

## AI agents (banyan-mcp)

`banyan-mcp` is an MCP server over stdio. Agents get these tools:
`search_notes` (Thai-segmented), `read_note`, `save_note`, `get_links`,
`list_notes`, `list_vaults` — deliberately no delete tool; erasing knowledge
stays a human action.

```json
// .mcp.json in your repo
{ "mcpServers": { "banyan": { "command": "banyan-mcp" } } }
```

Full setup and a `CLAUDE.md` recipe: [docs/AI-AGENT.md](docs/AI-AGENT.md)

## Architecture

```
<vault>/
  *.md            ← source of truth (Obsidian-compatible)
  .brain/
    graph.redb    ← forward/backlinks + mtimes + index version (redb)
    tantivy/      ← full-text index with the Thai newmm tokenizer (tantivy)
~/.config/banyan/
  registry.redb   ← vault name -> path, for cross-vault links
```

Delete `.brain/` any time — `banyan reindex` rebuilds everything from the
Markdown files. When the schema/tokenizer version changes, stale indexes are
rebuilt automatically.

## Development

```sh
cargo test                              # unit + integration tests
cargo clippy --all --all-targets -- -D warnings
cargo fmt --all -- --check
```

> Note: run `cd web && npm run build` before the first `cargo test` so the
> embedded-UI tests exercise a real build (they self-skip the UI part otherwise).

## Roadmap

- Go public + publish release binaries (download and run, no Rust/Node needed)
- User dictionary for newer Thai loanwords newmm doesn't know yet
- "Add vault" button in the web UI (no terminal); package as a desktop app via Tauri
- Cross-device sync / AI features (note summaries, ask-your-vault) — later, as an open-core layer

## License

[AGPL-3.0-only](LICENSE) — free to use and modify; if you offer it as a
service, share your changes.

Word-segmentation dictionary `words_th.txt` from
[PyThaiNLP](https://github.com/PyThaiNLP/pythainlp) (Apache-2.0).
