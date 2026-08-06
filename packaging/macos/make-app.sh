#!/usr/bin/env bash
# Assemble Samong.app around the samong-app binary.
#
# A .app bundle is a directory with a known shape, so this needs no Xcode, no
# signing identity and no extra tooling — which is why the bundle is built here
# in six lines of shell rather than through a packaging crate.
#
# Usage: make-app.sh <dir holding the built binaries> <output dir> <version>
set -euo pipefail

bin_dir="${1:?binary directory}"
out_dir="${2:?output directory}"
version="${3:?version}"

app="$out_dir/Samong.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

# The launcher is the executable the bundle runs. The CLI binaries travel beside
# the bundle rather than inside it: someone who wants `samong` on their PATH
# should not have to know that it lives inside an app bundle.
cp "$bin_dir/samong-app" "$app/Contents/MacOS/Samong"
chmod +x "$app/Contents/MacOS/Samong"

# The icon. Resolved from this script's own location rather than the working
# directory, so the bundle can be built from anywhere. Loudly required: a bundle
# whose CFBundleIconFile names a file that is not there falls back to the generic
# blank-page icon, and Finder gives no hint why.
icns="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/assets/icon/samong.icns"
[ -f "$icns" ] || { echo "missing $icns — run packaging/icons/make-icons.py" >&2; exit 1; }
cp "$icns" "$app/Contents/Resources/samong.icns"

# LSUIElement: no Dock icon and no menu bar. The interface is the browser window
# that opens; a Dock icon with no window behind it is a promise this bundle
# cannot keep, and clicking it would do nothing.
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Samong</string>
  <key>CFBundleDisplayName</key><string>Samong</string>
  <key>CFBundleIdentifier</key><string>dev.samong.app</string>
  <key>CFBundleExecutable</key><string>Samong</string>
  <key>CFBundleIconFile</key><string>samong</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

echo "built $app"
