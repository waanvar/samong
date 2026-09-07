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
# loop the project is for — and `samong` is what registers the vault it
# searches. Debug builds are enough and take a fraction of the time of release
# ones. Not fatal: a sandbox without them should still be able to work on the
# code.
echo "==> samong and samong-mcp (for .mcp.json)"
if cargo build --locked --bin samong --bin samong-mcp; then
  mkdir -p "$HOME/.local/bin"
  ln -sf "$PWD/target/debug/samong" "$HOME/.local/bin/samong"
  ln -sf "$PWD/target/debug/samong-mcp" "$HOME/.local/bin/samong-mcp"

  # A server that connects and then answers "no vaults registered" to every
  # query is the quiet-success failure this project keeps warning about: the
  # MCP tools appear to work, and an agent concludes the repo has no notes.
  # The registry is empty in a fresh container, so fill it here.
  #
  # This repo is its own vault — README.md, PLAN.md, CONTRIBUTING.md, docs/ and
  # the rest of the committed Markdown, which is what `scope` already resolves
  # to without any samong.toml. The index it writes lands in .brain/, which is
  # gitignored, so registering does not dirty the tree.
  #
  # Deliberately *not* under SAMONG_CONFIG_DIR: the server that .mcp.json starts
  # reads ~/.config/samong, so anything written elsewhere would be invisible to
  # it. That is safe only because this path is a throwaway container's, never a
  # person's machine — which is also why this script belongs in .devcontainer/
  # and nowhere a laptop would run it.
  echo "==> register this repo as a vault"
  if "$PWD/target/debug/samong" vault list | cut -f1 | grep -qx samong; then
    echo "vault \"samong\" is already registered"
  else
    # `vault add` fails on a name already taken, and swallowing that with
    # `|| true` would also swallow a real failure. Ask first instead.
    "$PWD/target/debug/samong" vault add samong "$PWD"
  fi
else
  echo "samong did not build; the MCP server in .mcp.json will start but find no vault." >&2
fi

echo "==> ready"
