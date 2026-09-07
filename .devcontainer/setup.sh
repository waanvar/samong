#!/usr/bin/env bash
# Prepare a fresh sandbox to work on Samong.
#
# This runs once per cloud session, before any agent gets to work. Everything
# here is a thing the four checks in CLAUDE.md need and that a bare container
# does not have. Anything slow and optional belongs in the session, not here.
set -euo pipefail

echo "==> rust components"
rustup component add clippy rustfmt

echo "==> cargo dependencies (--locked: the set that ships, not the newest)"
cargo fetch --locked

echo "==> web dependencies"
(cd web && npm ci)

# The UI is embedded into the binary at compile time, so an unbuilt web/dist
# means the first cargo build serves an empty interface. Build it once here and
# the session starts from a truthful state.
echo "==> web build"
(cd web && npm run build)

# samong-mcp gives the agent search over this repo's own notes — the knowledge
# loop the project is for. A debug build is enough and takes a fraction of the
# time of a release one. Not fatal: a sandbox without it should still be able to
# work on the code.
echo "==> samong-mcp (for .mcp.json)"
if cargo build --locked --bin samong-mcp; then
  mkdir -p "$HOME/.local/bin"
  ln -sf "$PWD/target/debug/samong-mcp" "$HOME/.local/bin/samong-mcp"
else
  echo "samong-mcp did not build; the MCP server in .mcp.json will not start." >&2
fi

echo "==> ready"
