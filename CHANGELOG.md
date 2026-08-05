# Changelog

Notable changes per release. Dates are the tag date.

The version starts at 0.3.0: two earlier generations of the index and API existed
during development but were never published, and the numbering keeps their
lineage rather than pretending this is the first shape the project took.

## Unreleased

_Nothing yet._

## 0.3.8

The registry entry v0.3.7 could not publish.

### Fixed

- **`server.json`'s description was 170 characters against a limit of 100**, so
  the MCP registry rejected it with a 422 and the listing never appeared. The
  bundle itself published fine — `samong-mcp.mcpb` is attached to 0.3.7, its
  macOS binary is universal, and its checksum verifies. Only the metadata was
  wrong.
- The limit was in the published schema the whole time
  (`ServerDetail.description.maxLength`). It was missed because the schema had
  been read for enumerations rather than validated against, so
  **`server.json` is now checked against the schema it names** — in CI, and again
  in the release workflow before anything is published. The two field limits that
  actually bit are asserted offline by a test as well.

## 0.3.7

Listed in the official MCP registry, and installable without a Rust toolchain.

### The MCP server is in the registry

`samong-mcp` is published to the [MCP registry](https://registry.modelcontextprotocol.io)
as **`io.github.waanvar/samong`**, so a client can find and install it without
being told where to look.

- Each release now also publishes **`samong-mcp.mcpb`** — an MCP bundle carrying
  the server for Linux, Windows, and both Mac architectures in one file, with a
  `.sha256` beside it. No toolchain, no npm wrapper package.
- The macOS binary inside is **universal** (`lipo`), because the bundle manifest
  has one `darwin` key and two architectures exist. The build fails rather than
  warns if it cannot make one: an arm64-only bundle installs cleanly on an Intel
  Mac and then cannot execute.
- Publishing authenticates with **GitHub OIDC** from the release workflow, so
  there is no long-lived registry token to store.

> Not published via `registryType: "cargo"`, though the registry supports it. A
> cargo entry has clients invoke the binary named after the crate — and this crate
> is `samong`, whose `samong` binary is the CLI, not the MCP server. Doing it
> properly means a separate thin crate; until then MCPB is both correct and
> toolchain-free.

### With Rust already installed

```sh
cargo install samong
```

Also new: the crate is publishable at all. `cargo package` honours `.gitignore`,
and both the built web UI and the Thai dictionary were being excluded — the second
of which meant the crate did not compile. See 0.3.6's notes for the shape of it;
this is the release where it works.

## 0.3.6

`samong update` works. It never had.

### Fixed

Self-updating was broken in every release from 0.3.0 to 0.3.5, in three separate
ways, and reported success anyway — which is why nobody noticed.

- **It printed "updated to <version>" while every binary had been skipped.** The
  loop caught each failure, printed it, and then announced success unconditionally.
  A command that cannot fail visibly is a command whose failures accumulate. It now
  says how many binaries were replaced, names the ones that were not, and exits
  non-zero when none were.
- **It looked for the binary at the root of the archive.** Every archive unpacks
  into one directory named after the release, so extraction failed with "specified
  file not found in archive".
- **On Windows it could not decompress the archive at all** — "Compression method
  not supported". The `self_update` dependency had `archive-zip`, which reads a zip
  but only decompresses *stored* entries; deflated ones need
  `compression-zip-deflate`, which was not enabled.
- **And it installed every binary over the running one.** `bin_install_path`
  defaults to the current executable, so updating three binaries in a loop
  overwrote the running one three times: `samong.exe` ended up being `samong-mcp`.

> **If you are on 0.3.5 or earlier, `samong update` cannot bring you here** — the
> broken updater is the thing being fixed. Download 0.3.6 by hand once, and
> updates work from then on.

Two tests now assert what could previously only fail on a user's machine: that the
in-archive path still matches what the release workflow builds, and that the
features needed to unpack a release are compiled in.

## 0.3.5

Download links that lead to a file.

### Fixed

- **The four download buttons on samong.dev all pointed at the releases page.**
  Each was labelled by platform and architecture and none of them delivered one:
  clicking "Windows · x86_64 · zip" landed on a list of eight files, for exactly
  the audience the double-click launcher exists for. A direct link needs the exact
  filename, and every filename carried the version.
- Each release now also publishes **an unversioned copy of every archive**, so
  `releases/latest/download/samong-x86_64-windows.zip` is a permanent link that
  resolves against whatever the newest release is — the website never has to know
  the version. Versioned names stay, because "which build is this" has to remain
  answerable from the filename alone, and each name gets its own `.sha256`
  (`sha256sum -c` matches on the filename recorded inside the file).
- The landing page linked `brand.html`, which the clean-URL rule redirects to
  `/brand` — a wasted round trip on every click. It links `/brand` directly now.

## 0.3.4

The release where Samong stops requiring a terminal, and where a vault becomes
something you can hand to another person.

> **0.3.3 was published and withdrawn.** Its Windows archive shipped the GUI
> launcher as `samong.exe`: the packaging step copied the launcher to
> `Samong.exe`, which on a case-insensitive filesystem *is* `samong.exe`, so it
> replaced the command-line tool — anyone who unpacked it and ran `samong search`
> would have got a browser window. The double-click copy is now **`Open
> Samong.exe`**, which cannot collide, and packaging **checks the staged archive
> before creating it**: every expected binary present, and no two
> differently-named binaries turning out to be the same file. Four jobs went green
> on the broken archive because nothing looked. The macOS and Linux archives were
> unaffected, and 0.3.4 is 0.3.3 plus that fix.

### Double-click to open it

Windows `Open Samong.exe`, macOS `Samong.app`, Linux `samong-app` with a
`.desktop` file. No terminal, no configuration, no account.

- **A first run with nothing to answer.** It creates a vault at
  `Documents/Samong`, writes two notes that link to each other, indexes them, and
  opens your browser. Two notes rather than one because the first thing you see is
  the map, and one note draws a single dot that demonstrates nothing.
- **A second double-click brings you back to the window that is already open**
  instead of starting a second server on the same vault.
- **It moves off a busy port rather than failing** — and checks that what is on
  port 3000 is actually Samong before pointing your browser at it.
- **The `⏻` button quits.** The server outlives the browser tab, so closing the
  tab is not the same as stopping the program; without this the only way out was
  the task manager.
- No console window on Windows, and no Dock icon on macOS: the interface is the
  browser window. If the launcher fails it writes `~/.config/samong/launcher.log`
  and opens it, because a windowless program has nowhere else to say anything.

> Neither the app bundle nor the `.exe` is code-signed. macOS **refuses** to open
> it on first launch — right-click → **Open** — and Windows SmartScreen may warn.
> Each archive ships a `DOUBLE-CLICK.md` that says so plainly.

### A vault can be published, installed, updated, and verified

- **`samong pack <dir>`** copies out the publishable part of a vault: in-scope
  `.md` files and `samong.toml`, nothing else. A whitelist, not a
  copy-then-delete — `.brain/` holds a full copy of every note body *and* the
  titles of notes you deleted, so a vault tidied up before publishing would have
  shipped the tidying too. Refuses to run until `[vault] license` says what
  people may do with the notes.
- **`samong vault install <git-url>`** clones someone else's vault in as
  **read-only reference notes**: same graph, same search, `[[links]]` from your
  own notes resolve into it. It wires the path into `scope.include` *and* into
  your `.gitignore`, with the reason written beside the rule — committing notes
  you bought into your own repository is a licence breach nobody chose to make.
- **`samong vault update [name]`** pulls new content. One vault whose access has
  lapsed does not stop the others.
- **`samong vault verify [name]`** answers "is this the vault its publisher
  published". Not a checksum file — a git checkout is already a Merkle tree, and a
  digest sitting beside the content it describes is not a security control.
  Instead: the commit signature, the signer **pinned at install** the way SSH pins
  a host key, and any local change to a copy that is supposed to be read-only —
  including files nobody published, since a stray `.md` dropped into an installed
  vault would appear in your search results credited to its author.
- An update **signed by a different key, or suddenly not signed at all, is
  refused before the merge** — nothing reaches your working tree or your index.
- Publishers should sign **commits**, not release tags: updates follow a branch,
  so a tag signature says nothing about the commit you just pulled.
- **`samong.toml`** is edited with a format-preserving parser, so the comments and
  ordering in a file you wrote by hand survive us writing to it.

### Search results say whose notes they are

A hit from an installed vault now carries that vault's name and its licence — in
the CLI, the web UI, the API, and the MCP tools. The moment worth protecting is
not the search; it is the paragraph somebody copies out of a result into work of
their own, after which nothing records where it came from. A vault that states no
licence reads `licence not stated`, because that is an answer rather than a gap.

The MCP `read_note` tool prefixes reference notes with their source for the same
reason: an agent is the most likely thing to lift a paragraph out of a bought
vault and drop it into a note of yours.

### Search finds notes by meaning, if you ask for it

- **Optional local semantic search.** Build with `--features semantic` and run
  `samong embed` to rank by meaning as well as by words, with a multilingual model
  so it works on Thai notes and not only English ones. Off by default and absent
  from the published binaries: it pulls in ONNX Runtime and downloads a 465 MB
  model, and "one binary, nothing to fetch" is a promise worth keeping for
  everyone who does not need this.
- Reference notes are excluded from embedding unless you pass `--reference`; on a
  430-note vault they were 95% of an eleven-minute run.
- **Search ranks by connectedness as well as relevance.** When the words cannot
  tell two notes apart, the one the rest of your notes point at comes first.

### Fixed

- **A `[[` with no closing `]]` no longer swallows the next real link.** The
  pattern could match across line breaks, so a note that *described* wikilinks —
  a bare `[[` in prose on one line, a real link two lines below — produced a graph
  node whose label was a paragraph of text. Wikilinks are now single-line, which
  is also Obsidian's rule. Renaming was fixed to match, so it can no longer miss
  some links while mangling others.

## 0.3.2

The first release with an archive for every platform — see the last item.

### The map is readable in a vault full of vendored docs

A vault pointed at a project root can hold 425 read-only notes from
`node_modules` against 5 of your own. At that ratio the graph stopped being a map
and became a uniform field of rings with your own knowledge lost inside it.

- **The graph now shows your notes plus one hop**: everything you wrote, and
  whatever those notes link to directly. A borrowed documentation page you
  actually cite belongs on your map; the other 423 do not.
- Reference notes are one click away — **the legend is the switch**, and it says
  how many are currently off the map. When they are shown they are drawn smaller
  and fainter, so your own notes stay in front.
- **A "not created yet" target only appears beside the note that names it.** They
  used to float unattached once their source was filtered out.
- Layout clusters by the folder a note actually sits in. It used to cluster by
  the top-level folder, which is `node_modules` for every vendored file, so every
  reference note landed in one group and the clustering said nothing.

### The note list is usable at depth

- **Chains of folders that contain nothing but each other collapse into one row.**
  `node_modules/next/dist/docs` was five rows deep before the first readable
  name, and the indentation truncated leaf labels to `0…`.

### Fixed

- **The Intel Mac archive is built again.** It asked for a `macos-13` runner,
  which GitHub has retired, so that job queued indefinitely and the archive was
  never published for 0.3.0 or 0.3.1. It now cross-compiles from the arm64
  runner.

## 0.3.1

The 0.3.0 binaries were built from a commit that predates the brand work below,
so this is the first build where what you download matches what the website
shows. No data format changed: the index and `samong.toml` are unchanged, and
0.3.0 vaults are read as-is.

- **The wordmark is outlined.** The name used to be live text set in Bai
  Jamjuree; it is now SVG paths, so it renders identically without the font
  present and can be handed to a printer as-is. The letterforms are the same
  ones — they were extracted from the font, not redrawn.
- **The brand is bigger in the app**, and sized from cap height rather than
  font-size so the mark and the name scale as one object.
- Brand assets — mark, wordmark and lockup, in colour and one-colour versions —
  with usage rules, at `site/brand.html`.
- Fixed a rendering bug where the wordmark showed only a fragment of one letter
  when referenced through an SVG `<use>`.

### Fixed for contributors

- `cargo test` no longer opens the real registry at `~/.config/samong`. Five test
  files were using it, which made parallel runs fight over redb's exclusive lock
  — and meant running the suite could modify vaults you actually use.
- Replaced a wall-clock assertion in the incremental-reindex test with a check on
  the work done, which is what the feature promises and does not depend on how
  busy the machine is.

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
