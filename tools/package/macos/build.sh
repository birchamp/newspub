#!/usr/bin/env bash
# Builds the macOS packages from the arm64 and x86_64 release binaries: a universal newpub.app,
# a drag-to-Applications .dmg, and a .pkg installer.
# Usage: tools/package/macos/build.sh <arm64 binary> <x86_64 binary> <version> <out dir> [arm64 newpub-agent] [x86_64 newpub-agent]
# Without an Apple Developer ID the app is ad-hoc signed, so macOS asks once before opening it
# (System Settings > Privacy & Security > Open Anyway).
set -euo pipefail
ARM=$1 X86=$2 VERSION=$3 OUT=$4 ARM_AGENT=${5:-} X86_AGENT=${6:-}
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../../.." && pwd)
WORK=$(mktemp -d)
mkdir -p "$OUT"

APP="$WORK/newpub.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/newpub" "$ARM" "$X86"
# The MCP server for AI agents ships inside the bundle (docs/agent.md).
if [ -n "$ARM_AGENT" ]; then lipo -create -output "$APP/Contents/MacOS/newpub-agent" "$ARM_AGENT" "$X86_AGENT"; fi
sed "s/@VERSION@/$VERSION/g" "$HERE/Info.plist" >"$APP/Contents/Info.plist"
printf 'APPL????' >"$APP/Contents/PkgInfo"

# Icon: an .iconset from the 1024 px PNG, then .icns.
SET="$WORK/newpub.iconset"
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  sips -z $s $s "$ROOT/assets/icon/newpub.png" --out "$SET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) "$ROOT/assets/icon/newpub.png" --out "$SET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns -o "$APP/Contents/Resources/newpub.icns" "$SET"

codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"
lipo -archs "$APP/Contents/MacOS/newpub"

# .dmg: the app next to a link to /Applications.
DMG="$WORK/dmg"
mkdir -p "$DMG"
cp -R "$APP" "$DMG/"
ln -s /Applications "$DMG/Applications"
hdiutil create -volname "newpub $VERSION" -srcfolder "$DMG" -ov -format UDZO "$OUT/newpub-${VERSION}-macos-universal.dmg"

# .pkg: installs newpub.app into /Applications.
PKGROOT="$WORK/pkgroot"
mkdir -p "$PKGROOT/Applications"
cp -R "$APP" "$PKGROOT/Applications/"
# Not relocatable: always install into /Applications, even if another copy exists elsewhere.
pkgbuild --analyze --root "$PKGROOT" "$WORK/components.plist"
plutil -replace 0.BundleIsRelocatable -bool NO "$WORK/components.plist"
pkgbuild --root "$PKGROOT" --component-plist "$WORK/components.plist" --identifier io.github.birchamp.newpub \
  --version "$VERSION" --install-location / "$OUT/newpub-${VERSION}-macos-universal.pkg"

ls -l "$OUT"
