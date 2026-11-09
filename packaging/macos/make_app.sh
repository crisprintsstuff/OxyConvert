#!/usr/bin/env bash
# Build OxyConvert.app (universal: Apple Silicon + Intel) and a .dmg.
#
# Usage (on macOS, from repo root, after building both targets):
#   cargo build --release --target aarch64-apple-darwin
#   cargo build --release --target x86_64-apple-darwin
#   packaging/macos/make_app.sh 0.1.0
#
set -euo pipefail

VERSION="${1:?usage: make_app.sh VERSION}"
NUMERIC="${VERSION%%-*}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/dist"
WORK="$(mktemp -d)"
APP="$WORK/OxyConvert.app"

mkdir -p "$OUT" "$APP/Contents/MacOS" "$APP/Contents/Resources"

# Universal binary from both architectures
lipo -create -output "$APP/Contents/MacOS/oxyconvert" \
  "$ROOT/target/aarch64-apple-darwin/release/oxyconvert" \
  "$ROOT/target/x86_64-apple-darwin/release/oxyconvert"
lipo -info "$APP/Contents/MacOS/oxyconvert"

# Icon: .iconset -> .icns
ICONSET="$WORK/oxybrush.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  if [ -f "$ROOT/packaging/icons/oxybrush-$s.png" ]; then
    cp "$ROOT/packaging/icons/oxybrush-$s.png" "$ICONSET/icon_${s}x${s}.png"
  fi
  if [ -f "$ROOT/packaging/icons/oxybrush-$((s * 2)).png" ]; then
    cp "$ROOT/packaging/icons/oxybrush-$((s * 2)).png" "$ICONSET/icon_${s}x${s}@2x.png"
  fi
done
iconutil -c icns -o "$APP/Contents/Resources/oxybrush.icns" "$ICONSET"

sed -e "s/@VERSION@/$VERSION/g" -e "s/@NUMERIC_VERSION@/$NUMERIC/g" \
  "$ROOT/packaging/macos/Info.plist.in" > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
cp "$ROOT/LICENSE-MIT" "$ROOT/LICENSE-APACHE" "$APP/Contents/Resources/"

# Ad-hoc signature: required for Apple Silicon
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

# Disk image with an Applications shortcut for drag-and-drop install
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
cp "$ROOT/README.md" "$STAGE/README.md"
DMG="$OUT/OxyConvert-$VERSION-macos-universal.dmg"

retry() {
  local attempt
  for attempt in 1 2 3 4 5; do
    if "$@"; then
      return 0
    fi
    echo "attempt $attempt of: $* failed; retrying in $((attempt * 5)) s" >&2
    sleep $((attempt * 5))
  done
  return 1
}
retry hdiutil create -volname "OxyConvert $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
retry hdiutil verify "$DMG"

rm -rf "$OUT/OxyConvert.app"
cp -R "$APP" "$OUT/OxyConvert.app"
echo "Built $DMG"
