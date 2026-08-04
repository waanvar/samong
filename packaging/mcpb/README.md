# The MCP bundle, and how Samong reaches the MCP registry

The registry stores metadata only; the artefact lives on GitHub Releases. Two
files describe it:

| File | What it is |
|---|---|
| `../../server.json` | the registry entry — server name, description, and where the bundle is |
| `manifest.json` | inside the bundle — what to execute, per platform |
| `build.sh` + `pack.py` | assemble the bundle from published release archives |

## Why MCPB and not cargo

The registry supports both for Rust. `registryType: "cargo"` distributes through
crates.io and expects the client to invoke the installed binary **by name derived
from the crate**. This project publishes one crate, `samong`, that produces four
binaries — and the one an MCP client must run is `samong-mcp`, not `samong`. A
cargo entry would point a client at the CLI.

Making that path work would mean publishing a second, thin crate whose only
binary is `samong-mcp`. Worth doing later; it is not free, because two crates
sharing one version number is a release-time trap of its own.

MCPB also happens to fit the direction the rest of this project took: it needs no
Rust toolchain, which is the same reason the double-click launcher exists.

## Why one bundle for four platforms

A `packages[]` entry in `server.json` has **no OS or architecture field**. Nothing
in it could tell a client which of four downloads to take, so the platform choice
has to live inside the bundle, where `platform_overrides` in `manifest.json`
expresses it.

macOS then needs care: there is one `darwin` key and two architectures. `lipo`
fuses the arm64 and x86_64 builds into a single universal binary. `build.sh`
**fails** rather than warns when `lipo` is unavailable — an arm64-only bundle
installs cleanly on an Intel Mac and then cannot execute, and a failure that only
appears on someone else's machine is the kind this project keeps paying for.

## Why the zip is written by Python

A zip entry carries Unix permissions in its external attributes. A server binary
that unpacks without the execute bit cannot be launched, and nothing would say
why. `pack.py` sets the mode explicitly, uses a fixed timestamp so identical
inputs give identical bytes, and asserts the result — every required file present,
every binary executable — before the bundle can be uploaded.

## Verification the registry performs

- The bundle URL must contain `mcp`. `samong-mcp.mcpb` satisfies this twice.
- `fileSha256` must be present. The release workflow computes it from the built
  bundle and writes it into `server.json` before publishing; the committed copy
  deliberately omits it, because a hash checked into git would be a claim about a
  file that does not exist yet.
- The `io.github.waanvar` namespace is proven by GitHub OIDC from the release
  workflow — no long-lived token to store or leak.

## Publishing by hand

Normally the release workflow does this. To do it manually:

```sh
brew install mcp-publisher     # or download from the registry's releases
mcp-publisher login github
mcp-publisher publish
```
