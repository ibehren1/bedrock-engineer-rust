#!/bin/bash
#
# Generate the app icon masters (build/icon.png, build/icon.iconset/) from a
# single source logo, composited onto a dark glossy rounded-square background
# via compose-icon.py, then regenerate the Tauri bundle icons
# (src-tauri/app/icons/: .icns, .ico and PNGs) from the 1024px master.
#
# Usage:
#   ./make-icon.sh path/to/source-logo.png
#
# Requirements: macOS (sips), python3 + Pillow, npm dependencies installed
# (the Tauri CLI comes from @tauri-apps/cli).
#
set -e

SRC=$1
if [ -z "$SRC" ]; then
  echo "usage: $0 <source-logo.png>" >&2
  exit 1
fi

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
MASTER="$(mktemp -t icon-master).png"
ICONSET="$HERE/icon.iconset"
TAURI_ICONS="$REPO/src-tauri/app/icons"

# 1) Composite the logo onto the dark glossy background at high resolution.
python3 "$HERE/compose-icon.py" "$SRC" "$MASTER" 1024

# 2) Repo master (512x512).
sips -z 512 512 "$MASTER" --out "$HERE/icon.png" >/dev/null

# 3) Iconset with every size (the 1024px icon_512x512@2x.png is the Tauri source).
mkdir -p "$ICONSET"
sips -z 16 16     "$MASTER" --out "$ICONSET/icon_16x16.png"      >/dev/null
sips -z 32 32     "$MASTER" --out "$ICONSET/icon_16x16@2x.png"   >/dev/null
sips -z 32 32     "$MASTER" --out "$ICONSET/icon_32x32.png"      >/dev/null
sips -z 64 64     "$MASTER" --out "$ICONSET/icon_32x32@2x.png"   >/dev/null
sips -z 128 128   "$MASTER" --out "$ICONSET/icon_128x128.png"    >/dev/null
sips -z 256 256   "$MASTER" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256   "$MASTER" --out "$ICONSET/icon_256x256.png"    >/dev/null
sips -z 512 512   "$MASTER" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512   "$MASTER" --out "$ICONSET/icon_512x512.png"    >/dev/null
sips -z 1024 1024 "$MASTER" --out "$ICONSET/icon_512x512@2x.png" >/dev/null
rm -f "$MASTER"

# 4) Tauri bundle icons. The app ships desktop bundles only, so drop the mobile sets.
(cd "$REPO" && npx tauri icon "$ICONSET/icon_512x512@2x.png" -o "$TAURI_ICONS")
rm -rf "$TAURI_ICONS/android" "$TAURI_ICONS/ios"

echo "Done: build/icon.png, build/icon.iconset/, src-tauri/app/icons/"
