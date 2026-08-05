#!/usr/bin/env bash
# Refuse to publish unless the only thing git does not have is the built web UI.
#
# `cargo publish` rejects a package containing files git does not track, and
# `include` in Cargo.toml deliberately packages `web/dist`, which is gitignored.
# So publishing this crate always needs `--allow-dirty` — and that flag, used
# bare, also silences the real warning it exists for: source edits that were never
# committed going out in a release nobody can reproduce.
#
# Written after `cargo publish --locked` failed with "55 files in the working
# directory contain changes that were not yet committed" — all 55 under web/dist.
# The CI dry-run had passed because it was already using `--allow-dirty`: a
# dry-run that cannot fail the way the real command fails checks nothing.
#
# Two separate things are checked, and earlier versions of this script got each of
# them wrong in a way worth recording:
#
#   1. Uncommitted *source*. That is what `--allow-dirty` would hide.
#   2. The files cargo objects to. Those are **ignored**, not untracked, so
#      `git status --porcelain` reports zero while cargo counts 55. And asking git
#      for every ignored path instead lists `target/` and `web/node_modules/`,
#      which are not in the package at all — the question is only about files
#      cargo would actually ship, so the list has to come from cargo.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

dirty=$(git status --porcelain --untracked-files=all | sed 's/^...//' | grep -v '^$' || true)
if [ -n "$dirty" ]; then
  {
    echo "the working tree has uncommitted changes — commit or stash them first:"
    echo "$dirty" | sed 's/^/  /'
  } >&2
  exit 1
fi

# cargo's own view of the package, normalised: it prints backslashes on Windows,
# and adds two files that exist only inside the archive.
listing=$(cargo package --list --allow-dirty --offline 2>/dev/null |
  tr -d '\r' | tr '\\' '/' |
  grep -vE '^(Cargo\.toml\.orig|\.cargo_vcs_info\.json)$')

if [ -z "$listing" ]; then
  echo "cargo package --list produced nothing" >&2
  exit 1
fi

untracked_in_package=""
while IFS= read -r file; do
  [ -n "$file" ] || continue
  if ! git ls-files --error-unmatch "$file" >/dev/null 2>&1; then
    untracked_in_package="${untracked_in_package}${file}"$'\n'
  fi
done <<EOF
$listing
EOF

stray=$(echo "$untracked_in_package" | grep -v '^$' | grep -v '^web/dist/' || true)
if [ -n "$stray" ]; then
  {
    echo "the package would carry files git does not have, outside web/dist:"
    echo "$stray" | sed 's/^/  /'
    echo "commit them, or take them out of the include list in Cargo.toml."
  } >&2
  exit 1
fi

count=$(echo "$untracked_in_package" | grep -c '^web/dist/' || true)
if [ "$count" -eq 0 ]; then
  echo "the package carries no built UI — run 'npm run build' in web/ first" >&2
  exit 1
fi

echo "no uncommitted source; $count generated file(s) under web/dist will be packaged"
