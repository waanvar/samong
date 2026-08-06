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

# The icon, per platform. A missing icon does not fail anything at build time and
# does not stop the program running — it just makes the bundle look unfinished in
# the one place a non-developer meets it, which is precisely the class of defect
# this script exists for.
case "$target" in
  *windows*)
    # On Windows the icon lives *inside* the .exe, so there is no file to look
    # for — the resource directory has to be read. build.rs only warns when a
    # resource compiler is missing, precisely so a contributor is not blocked by
    # a cosmetic resource; this is the loud half of that bargain, and it is why a
    # release cannot quietly ship binaries with the generic icon.
    # Probed by running it, not by `command -v`. Windows ships an App Execution
    # Alias called python3.exe that exists on PATH and prints "Python was not
    # found" when invoked — so a presence test picks an interpreter that cannot
    # run anything, and the check fails for a reason that has nothing to do with
    # the icon.
    py=""
    for cand in python3 python py; do
      if "$cand" -c "import sys" >/dev/null 2>&1; then py="$cand"; break; fi
    done
    [ -n "$py" ] || fail "no working Python found to read the executables' resources"
    "$py" "$(dirname "${BASH_SOURCE[0]}")/icons/check-exe-icon.py" \
      --require 16,32,48,256 \
      "$stage/samong.exe" "$stage/samong-app.exe" "$stage/samong-server.exe" \
      "$stage/samong-mcp.exe" "$stage/Open Samong.exe" \
      || fail "the Windows executables do not all carry the embedded icon"
    ;;
  *macos*)
    icns="$stage/Samong.app/Contents/Resources/samong.icns"
    [ -f "$icns" ] || fail "Samong.app has no icon at Contents/Resources/samong.icns"
    # CFBundleIconFile names "samong"; if the plist and the file disagree, macOS
    # shows the generic blank-page icon and says nothing.
    grep -q "<string>samong</string>" "$stage/Samong.app/Contents/Info.plist" \
      || fail "Info.plist does not declare CFBundleIconFile samong"
    head -c 4 "$icns" | grep -q icns || fail "samong.icns is not an ICNS file"
    ;;
  *linux*)
    for size in 16 32 48 128 256; do
      png="$stage/icons/hicolor/${size}x${size}/apps/samong.png"
      [ -f "$png" ] || fail "the ${size}px icon is missing from the archive"
    done
    grep -q '^Icon=samong$' "$stage/samong.desktop" \
      || fail "samong.desktop does not point at the installed icon"
    ;;
esac

echo "packaging checks passed for $target"
