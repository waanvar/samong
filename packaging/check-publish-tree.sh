#!/usr/bin/env bash
# Refuse to publish unless the only uncommitted thing is the built web UI.
#
# `cargo publish` rejects a package containing files git does not track, and
# `include` in Cargo.toml deliberately pulls in `web/dist`, which is gitignored.
# So publishing this crate always needs `--allow-dirty` — and that flag, used
# bare, also silences the real warning it exists for: source edits that were never
# committed going out in a release nobody can reproduce.
#
# This narrows it. `--allow-dirty` stays, but only after proving that every dirty
# path is generated UI. Anything else and the publish stops.
#
# Written after `cargo publish --locked` failed with "55 files in the working
# directory contain changes that were not yet committed" — all 55 under web/dist.
# The CI dry-run had passed because it was already using `--allow-dirty`.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# --porcelain covers staged, unstaged and untracked in one list. Untracked
# directories are collapsed by default, so ask for the files inside them.
dirty="$(git status --porcelain --untracked-files=all)"

unexpected="$(printf '%s\n' "$dirty" \
  | sed 's/^...//' \
  | grep -v '^$' \
  | grep -v '^web/dist/' \
  || true)"

if [ -n "$unexpected" ]; then
  echo "the working tree has changes outside web/dist — commit or stash them first:" >&2
  printf '%s\n' "$unexpected" | sed 's/^/  /' >&2
  exit 1
fi

count="$(printf '%s\n' "$dirty" | grep -c '^' || true)"
echo "clean apart from the built web UI ($count generated file(s) under web/dist)"
