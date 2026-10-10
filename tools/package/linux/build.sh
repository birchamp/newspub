#!/usr/bin/env bash
# Builds the Linux packages from a release binary: an AppImage (runs on most distributions), a .deb
# (Debian, Ubuntu, Mint) and a plain .tar.gz.
# Usage: tools/package/linux/build.sh <newpub binary> <version> <out dir> [newpub-agent binary]
set -euo pipefail
BIN=$1 VERSION=$2 OUT=$3 AGENT=${4:-}
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../../.." && pwd)
ICON="$ROOT/assets/icon/newpub.png"
WORK=$(mktemp -d)
mkdir -p "$OUT"

# Shared file tree: /usr/bin/newpub, desktop entry, icon, MIME type for .npub.
stage() {
  local d=$1
  install -Dm755 "$BIN" "$d/usr/bin/newpub"
  [ -n "$AGENT" ] && install -Dm755 "$AGENT" "$d/usr/bin/newpub-agent"
  install -Dm644 "$HERE/newpub.desktop" "$d/usr/share/applications/newpub.desktop"
  install -Dm644 "$HERE/newpub-mime.xml" "$d/usr/share/mime/packages/newpub.xml"
  install -Dm644 "$ROOT/assets/icon/newpub-256.png" "$d/usr/share/icons/hicolor/256x256/apps/newpub.png"
  install -Dm644 "$ICON" "$d/usr/share/icons/hicolor/1024x1024/apps/newpub.png"
  install -Dm644 "$ROOT/LICENSE" "$d/usr/share/doc/newpub/LICENSE"
  cp -r "$ROOT/assets/fonts/licenses" "$d/usr/share/doc/newpub/font-licenses"
}

# .deb
DEB="$WORK/deb"
stage "$DEB"
mkdir -p "$DEB/DEBIAN"
SIZE=$(du -sk "$DEB/usr" | cut -f1)
cat >"$DEB/DEBIAN/control" <<EOF
Package: newpub
Version: $VERSION
Section: graphics
Priority: optional
Architecture: amd64
Installed-Size: $SIZE
Depends: libc6, libgcc-s1, libxkbcommon0, libxkbcommon-x11-0, libgl1, libegl1, libx11-6, libxcursor1, libxi6, libxrandr2, libwayland-client0, libwayland-cursor0, libwayland-egl1
Maintainer: newpub contributors <noreply@users.noreply.github.com>
Description: Desktop publishing for newsletters, bulletins, flyers and booklets
 newpub lays out text and pictures on pages, prints booklets and exports PDF.
EOF
cat >"$DEB/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
command -v update-mime-database >/dev/null && update-mime-database /usr/share/mime || true
command -v update-desktop-database >/dev/null && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
EOF
chmod 755 "$DEB/DEBIAN/postinst"
dpkg-deb --root-owner-group --build "$DEB" "$OUT/newpub_${VERSION}_amd64.deb"

# AppImage
APPDIR="$WORK/newpub.AppDir"
stage "$APPDIR"
cp "$HERE/newpub.desktop" "$APPDIR/newpub.desktop"
cp "$ROOT/assets/icon/newpub-256.png" "$APPDIR/newpub.png"
cat >"$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE=$(dirname "$(readlink -f "$0")")
exec "$HERE/usr/bin/newpub" "$@"
EOF
chmod 755 "$APPDIR/AppRun"
TOOL="$WORK/appimagetool"
curl -fsSL -o "$TOOL" https://github.com/AppImage/appimagetool/releases/download/1.9.0/appimagetool-x86_64.AppImage
curl -fsSL -o "$WORK/runtime" https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64
chmod +x "$TOOL"
# Needs mksquashfs and desktop-file-validate on PATH (squashfs-tools, desktop-file-utils).
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --runtime-file "$WORK/runtime" "$APPDIR" \
  "$OUT/newpub-${VERSION}-linux-x86_64.AppImage"

# .tar.gz
TAR="$WORK/newpub-$VERSION"
mkdir -p "$TAR"
install -m755 "$BIN" "$TAR/newpub"
[ -n "$AGENT" ] && install -m755 "$AGENT" "$TAR/newpub-agent"
cp "$HERE/newpub.desktop" "$ROOT/assets/icon/newpub-256.png" "$ROOT/LICENSE" "$TAR/"
tar -C "$WORK" -czf "$OUT/newpub-${VERSION}-linux-x86_64.tar.gz" "newpub-$VERSION"

ls -l "$OUT"
