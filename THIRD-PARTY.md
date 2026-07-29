# Third-party components

Samong is [Apache-2.0](LICENSE). It ships or links the following third-party
work; every item below is under a license compatible with that.

## Bundled data

### `assets/words_th.txt` — Thai word-segmentation dictionary

- Source: [PyThaiNLP](https://github.com/PyThaiNLP/pythainlp)
- License: Apache-2.0
- Copyright: PyThaiNLP contributors

This 62,000-word dictionary is embedded in the binary and is what makes Thai
search work — it lets the tokenizer split unspaced Thai text into real words, so
a query matches mid-sentence. Samong would not do its main job without it.

### Fonts

All three are **SIL Open Font License 1.1**, which permits commercial use,
embedding, and putting text set in them on printed goods — it restricts
redistributing the *font software*, not the rendered output. Because Samong does
redistribute the font files (in `web/dist`, inside the `samong-server` binary,
and in `site/fonts/`), the licence text travels with them: see
`site/fonts/LICENSE-*.txt`.

| Font | Used for | Copyright | License |
|---|---|---|---|
| [Bai Jamjuree](https://github.com/cadsondemak/Bai-Jamjuree) | display / wordmark | Cadson Demak | OFL-1.1 |
| [IBM Plex Sans Thai](https://github.com/IBM/plex) | body text, Thai and Latin | IBM Corp. | OFL-1.1 |
| [IBM Plex Mono](https://github.com/IBM/plex) | paths, counts, code | IBM Corp. | OFL-1.1 |

The OFL's Reserved Font Name clause means a *modified* font must be renamed.
Samong ships these unmodified; the logo mark is drawn geometry and depends on no
font at all.

## Rust dependencies

Resolved versions and their license texts are in `Cargo.lock`; the direct
dependencies are:

| Crate | Purpose | License |
|---|---|---|
| [nlpo3](https://github.com/PyThaiNLP/nlpo3) | Thai word segmentation (newmm) | Apache-2.0 |
| [tantivy](https://github.com/quickwit-oss/tantivy) | Full-text search index | MIT |
| [redb](https://github.com/cberner/redb) | Embedded key-value store (link graph) | Apache-2.0 / MIT |
| [axum](https://github.com/tokio-rs/axum) | HTTP + WebSocket server | MIT |
| [tokio](https://github.com/tokio-rs/tokio) | Async runtime | MIT |
| [clap](https://github.com/clap-rs/clap) | CLI parsing | Apache-2.0 / MIT |
| [ignore](https://github.com/BurntSushi/ripgrep) | gitignore-aware directory walking | Unlicense / MIT |
| [notify](https://github.com/notify-rs/notify) | Filesystem watching | Artistic-2.0 / CC0-1.0 |
| [blake3](https://github.com/BLAKE3-team/BLAKE3) | Content hashing | Apache-2.0 / CC0-1.0 |
| [serde](https://github.com/serde-rs/serde) / [serde_json](https://github.com/serde-rs/json) | Serialization | Apache-2.0 / MIT |
| [toml](https://github.com/toml-rs/toml) | Config parsing | Apache-2.0 / MIT |
| [regex](https://github.com/rust-lang/regex) | Wikilink parsing | Apache-2.0 / MIT |
| [anyhow](https://github.com/dtolnay/anyhow) | Error handling | Apache-2.0 / MIT |
| [rust-embed](https://github.com/pyrossh/rust-embed) | Embedding the web UI in the binary | MIT |
| [self_update](https://github.com/jaemk/self_update) | `samong update` | MIT |
| [open](https://github.com/Byron/open-rs) | Opening the browser on server start | MIT |
| [tower-http](https://github.com/tower-rs/tower-http) | HTTP middleware | MIT |

To regenerate a full, exact list including transitive dependencies:

```sh
cargo install cargo-license && cargo license
```

## Web UI dependencies

The web UI is built with React and Vite and bundled into the server binary.
Exact versions and licenses are in `web/package-lock.json`.

## Not covered by the license

The name **"Samong"** and the project logo are not licensed under Apache-2.0.
See the README for what that means in practice — in short: fork freely, rename
what you ship.
