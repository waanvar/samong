#!/usr/bin/env bash
# Check a staged release directory before it is archived.
#
# Written because v0.3.3 shipped a Windows archive whose `samong.exe` was the GUI
# launcher rather than the CLI. The packaging step copied `samong-app.exe` to
# `Samong.exe`, and on the case-insensitive filesystem of the Windows runner that
# *is* `samong.exe` — so `cp` silently replaced the command-line tool. Nothing
# failed, every job went green, and the defect was only visible by reading the
# subsystem flag out of the published binary.
#
# A packaging step with no assertions is a packaging step that ships whatever it
# happened to produce. These are the assertions.
#
# Usage: verify-stage.sh <staged dir> <target name>
set -euo pipefail

stage="${1:?staged directory}"
target="${2:?target name}"
ext=""
case "$target" in *windows*) ext=".exe" ;; esac

fail() {
  echo "packaging check failed: $*" >&2
  exit 1
}

for name in samong samong-server samong-mcp samong-app; do
  [ -f "$stage/$name$ext" ] || fail "$name$ext is missing from the archive"
done

# The CLI and the launcher are different programs. Identical bytes means one
# overwrote the other, which is exactly how the v0.3.3 Windows archive broke.
cli=$(sha256sum "$stage/samong$ext" | cut -d' ' -f1)
app=$(sha256sum "$stage/samong-app$ext" | cut -d' ' -f1)
[ "$cli" != "$app" ] || fail "samong$ext and samong-app$ext are the same file — a name collision overwrote one of them"

# And the thing a person is meant to double-click has to be there too.
case "$target" in
  *windows*) [ -f "$stage/Open Samong.exe" ] || fail "the double-click launcher is missing" ;;
  *macos*)   [ -x "$stage/Samong.app/Contents/MacOS/Samong" ] || fail "Samong.app has no executable" ;;
  *linux*)   [ -f "$stage/samong.desktop" ] || fail "samong.desktop is missing" ;;
esac

echo "packaging checks passed for $target"
