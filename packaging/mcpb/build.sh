#!/usr/bin/env bash
# Assemble samong-mcp.mcpb — one bundle that works on all four platforms.
#
# An MCPB bundle is a zip holding `manifest.json` plus the server. It is what the
# MCP registry accepts for a prebuilt binary, and it is why publishing there
# needs no npm wrapper package.
#
# # Why one bundle and not four
#
# `server.json` lists packages in an array, but a package entry has no OS or
# architecture field — nothing in it tells a client which of four downloads to
# take. So the platform choice has to live *inside* the bundle, where
# `platform_overrides` in the manifest expresses it.
#
# That leaves macOS, which has two architectures and only one `darwin` key. The
# answer is a universal binary: `lipo` fuses the arm64 and x86_64 builds into one
# file that runs on both. Without it, half of Mac users would get a bundle whose
# binary cannot execute.
#
# Inputs are the published release archives rather than freshly built binaries:
# the bundle then contains exactly the bytes users download, not a rebuild that
# merely ought to match.
#
# Usage: build.sh <version, e.g. 0.3.6> <dir holding the release archives> <output dir>
set -euo pipefail

version="${1:?version without a leading v}"
archives="${2:?directory holding the release archives}"
out_dir="${3:?output directory}"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/server" "$out_dir"

extract() { # <archive> <path inside> <destination>
  local archive="$1" inside="$2" dest="$3"
  case "$archive" in
    *.zip) unzip -p "$archive" "$inside" > "$dest" ;;
    *.tar.gz) tar xzf "$archive" -O "$inside" > "$dest" ;;
    *) echo "unknown archive type: $archive" >&2; exit 1 ;;
  esac
  [ -s "$dest" ] || { echo "extracted nothing from $archive:$inside" >&2; exit 1; }
  chmod +x "$dest"
}

v="samong-v${version}"
extract "$archives/${v}-x86_64-linux.tar.gz"   "${v}-x86_64-linux/samong-mcp"   "$work/server/samong-mcp"
extract "$archives/${v}-x86_64-windows.zip"    "${v}-x86_64-windows/samong-mcp.exe" "$work/server/samong-mcp.exe"
extract "$archives/${v}-aarch64-macos.tar.gz"  "${v}-aarch64-macos/samong-mcp"  "$work/mcp-arm64"
extract "$archives/${v}-x86_64-macos.tar.gz"   "${v}-x86_64-macos/samong-mcp"   "$work/mcp-x86_64"

# One file for both Mac architectures, because the manifest has a single `darwin`
# key and no way to express two of them.
#
# `lipo` exists only on macOS, so this script belongs on a macOS runner. Missing
# it is an error rather than a warning: a bundle carrying only the arm64 slice
# installs cleanly on an Intel Mac and then fails to execute, which is the class
# of silent defect this project has already shipped more than once.
if command -v lipo >/dev/null 2>&1; then
  lipo -create -output "$work/server/samong-mcp-macos" "$work/mcp-arm64" "$work/mcp-x86_64"
  chmod +x "$work/server/samong-mcp-macos"
  echo "macOS: universal binary ($(lipo -archs "$work/server/samong-mcp-macos"))"
elif [ "${SAMONG_ALLOW_NO_LIPO:-}" = "1" ]; then
  echo "WARNING: no lipo — arm64 only, opted in via SAMONG_ALLOW_NO_LIPO" >&2
  cp "$work/mcp-arm64" "$work/server/samong-mcp-macos"
  chmod +x "$work/server/samong-mcp-macos"
else
  echo "lipo is required to fuse the two macOS builds into one binary." >&2
  echo "Run this on macOS, or set SAMONG_ALLOW_NO_LIPO=1 for a local arm64-only test." >&2
  exit 1
fi
rm -f "$work/mcp-arm64" "$work/mcp-x86_64"

sed "s/__VERSION__/${version}/" "$here/manifest.json" > "$work/manifest.json"
grep -q '__VERSION__' "$work/manifest.json" && { echo "version was not substituted" >&2; exit 1; }
# `python3` is the name on CI runners; a Windows install often exposes only
# `python`, where bare `python3` hits the Microsoft Store shim and "succeeds"
# while validating nothing.
py=""
for candidate in python3 python; do
  if "$candidate" -c "" >/dev/null 2>&1; then py="$candidate"; break; fi
done
[ -n "$py" ] || { echo "need python to validate the manifest" >&2; exit 1; }
"$py" -c "import json,sys; json.load(open(sys.argv[1]))" "$work/manifest.json" \
  || { echo "manifest.json is not valid JSON after substitution" >&2; exit 1; }

bundle="$out_dir/samong-mcp.mcpb"
# Packing and verifying live in pack.py: it sets the execute bit explicitly and
# asserts the result, neither of which the `zip` command can be relied on to do
# identically across runners.
"$py" "$here/pack.py" "$work" "$bundle"

echo "built $bundle ($(wc -c < "$bundle") bytes)"
