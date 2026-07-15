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

Requires [Rust](https://rustup.rs) (stable) and [Node.js](https://nodejs.org)
20+ (only for the web UI).

```sh
git clone https://github.com/waanvar/banyan.git
cd banyan
cargo build --release              # banyan + banyan-server in target/release/
cd web && npm install && npm run build   # (optional) build the web UI
```

Or install the CLI straight from git:

```sh
cargo install --git https://github.com/waanvar/banyan banyan
```

## Quickstart

```sh
mkdir my-vault && cd my-vault
banyan new "My First Note"         # create + index
banyan vault add my-vault .        # register in ~/.config/banyan
banyan-server                      # web UI at http://127.0.0.1:3000
```

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

## Web UI

An original design — banyan-tree identity, not an Obsidian clone — with
first-class Thai typography (IBM Plex Sans Thai, bundled, works offline).

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
| `GET /api/search?q=&vault=` | Search (omit `vault` for all vaults) |
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
cargo test                              # 57 tests (unit + integration)
cargo clippy --all --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Roadmap

- User dictionary for newer Thai loanwords newmm doesn't know yet
- Desktop app via Tauri
- Cross-device sync / AI features — later, as an open-core layer

## License

[AGPL-3.0-only](LICENSE) — free to use and modify; if you offer it as a
service, share your changes.

Word-segmentation dictionary `words_th.txt` from
[PyThaiNLP](https://github.com/PyThaiNLP/pythainlp) (Apache-2.0).
