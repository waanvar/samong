#!/bin/bash
# Claude Code cloud sessions do not use the devcontainer, so its onCreateCommand
# never runs there — and the sandbox started without `samong-mcp` on PATH. Every
# such session's .mcp.json server failed with "Executable not found", and agents
# lost search over this repo's own notes: the knowledge loop the project is for.
#
# The setup itself stays in one place. This hook only runs .devcontainer/setup.sh,
# so the two environments cannot drift into two different ideas of what a ready
# sandbox is.
#
# Remote only: setup.sh registers this repo in ~/.config/samong, which is right
# for a throwaway container and wrong for a person's machine (AGENTS.md: never
# touch the real registry).
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-$(dirname "$0")/../..}"
bash .devcontainer/setup.sh
