# Changelog

Notable changes per release. Dates are the tag date.

The version starts at 0.3.0: two earlier generations of the index and API existed
during development but were never published, and the numbering keeps their
lineage rather than pretending this is the first shape the project took.

## 0.3.0 — first public release

The first build published as a downloadable binary. Everything below is new to
anyone outside the project.

### Knowledge base

- Notes are plain Markdown, fully Obsidian-compatible (`[[wikilink]]` and
  `[[wikilink|alias]]`). The `.md` files are the only source of truth; every
  index lives in `<vault>/.brain/` and can be rebuilt with `samong reindex`.
- **Thai full-text search that segments words.** Thai has no spaces between
  words, so ordinary search treats a sentence as one token. Samong segments with
  the newmm dictionary via [nlpo3](https://github.com/PyThaiNLP/nlpo3), so
  searching `ตลาดหลักทรัพย์` matches inside
  `ตลาดหลักทรัพย์แห่งประเทศไทยเปิดทำการ`, with the matched tokens highlighted.
- Link graph in [redb](https://github.com/cberner/redb), search in
  [tantivy](https://github.com/quickwit-oss/tantivy).
- Multi-vault: link across projects with `[[vault-name/note]]`; cross-vault
  backlinks resolve at query time with no cross-vault index writes.
- Incremental reindex compares mtime first, then a blake3 content hash — so a
  `git checkout` that rewrites every mtime does not reindex the vault.

### What counts as a note

- **A note is a `.md` file you would commit.** Point `samong vault add` at a
  project root: `.gitignore` is respected, dependency directories
  (`node_modules`, `vendor`, `site-packages`, `__pycache__`, `Pods`,
  `bower_components`) are always skipped, and so is every dot-directory.
- Scope is decided only by files committed inside the vault — `samong.toml`,
  `.samongignore`, `.gitignore`. Per-machine sources (global gitignore,
  `.git/info/exclude`, parent-directory `.gitignore`) are deliberately ignored so
  the same commit indexes identically on every machine.
- `scope.include` pulls in documentation that git does not track — the docs a
  dependency ships, for instance. Those become **reference notes**: same vault,
  same index, but read-only, since the files belong to a dependency and any edit
  would be erased on the next install.
- `samong doctor` reports what was indexed, what was skipped and why, which
  include roots are missing on this machine, and which titles are ambiguous.

### Addressing

- Notes are identified by their **vault-relative path**, not by title. One repo
  can hold twenty files called `README.md`; a title cannot address them and a
  title-keyed index silently collapsed them.
- The HTTP API and the MCP tools both address notes by path. Search results carry
  the path so a caller never has to guess which file matched.

### Surfaces

- **CLI**: `new`, `edit`, `rename`, `delete`, `links`, `orphans`, `broken`,
  `search`, `graph`, `list`, `reindex`, `watch`, `vault`, `doctor`, `update`.
- **Local API + web UI** (`samong-server`), bound to `127.0.0.1` only. The web UI
  is embedded in the binary, so it ships as one file.
- **MCP server** (`samong-mcp`) over stdio with six tools: `list_vaults`,
  `list_notes`, `read_note`, `save_note`, `search_notes`, `get_links`.
  Deliberately no delete tool — an agent's memory should accumulate.
  `search_notes` takes a `limit` that is a total across vaults, because every hit
  is context the agent pays for on every later turn.
- The web UI puts the **graph** at the centre: a vault is a shape, and searching
  dims everything that does not match, so a query becomes a place.
- The interface is in **English or Thai**, picked from `?lang=`, a saved choice,
  or the browser, and switchable from the header. English is the default: Thai
  segmentation is what Samong is good at, but it should not be what you have to
  read to use it. The CLI and MCP server are English only.

### Licence

Apache-2.0. The name and logo are not covered by it — fork freely, rename what
you ship. Third-party attribution is in `THIRD-PARTY.md`.

### Known limitations

- Binaries are **not code-signed**. macOS Gatekeeper blocks unsigned downloads
  outright and Windows SmartScreen warns; see the README for how to open them.
- No authentication: `samong-server` is local-only by design.
- `read_note` / `save_note` address notes by path, but a `[[wikilink]]` still
  names a title, so an ambiguous title resolves to the first match in path order.
  `samong doctor` reports those.
