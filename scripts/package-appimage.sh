#!/usr/bin/env bash
# Build the 521C desktop AppImage (issue #8 baseline artifact).
#
# Usage: scripts/package-appimage.sh
#
# Inputs:  native workspace (release build of the five21c-desktop crate),
#          packaging/linux metadata, crate icon assets.
# Output:  native/dist/521C-<version>-x86_64.AppImage
#
# appimagetool resolution order: $APPIMAGETOOL, PATH, ~/.cache/521c-tools.
# The tool is official AppImage project release tooling; it is NOT vendored in
# the repository. See docs/DEVELOPMENT.md -> "Desktop app / AppImage".

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NATIVE="$ROOT/native"
ASSETS="$NATIVE/crates/521c-desktop/assets"
META="$ROOT/packaging/linux"
DIST="$NATIVE/dist"
APPDIR="$NATIVE/target/AppDir"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$NATIVE/crates/521c-desktop/Cargo.toml" | head -1)"
[ -n "$VERSION" ] || { echo "could not read crate version" >&2; exit 1; }

# 1. Release build -----------------------------------------------------------
echo "==> cargo build --release -p five21c-desktop"
(cd "$NATIVE" && cargo build --release -p five21c-desktop)
BIN="$NATIVE/target/release/521c"
[ -x "$BIN" ] || { echo "release binary not found: $BIN" >&2; exit 1; }

# 2. AppDir assembly ----------------------------------------------------------
echo "==> assembling AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/metainfo"

install -m 0755 "$BIN" "$APPDIR/usr/bin/521c"
ln -s usr/bin/521c "$APPDIR/AppRun"

install -m 0644 "$META/521c.desktop" "$APPDIR/521c.desktop"
install -m 0644 "$META/521c.desktop" "$APPDIR/usr/share/applications/521c.desktop"
install -m 0644 "$META/io.github.pedro-labsabs.521c.metainfo.xml" \
    "$APPDIR/usr/share/metainfo/io.github.pedro-labsabs.521c.metainfo.xml"

for size in 16 32 48 64 128 256 512; do
    src="$ASSETS/icons/521c_${size}.png"
    [ -f "$src" ] || { echo "missing icon: $src" >&2; exit 1; }
    mkdir -p "$APPDIR/usr/share/icons/hicolor/${size}x${size}/apps"
    install -m 0644 "$src" "$APPDIR/usr/share/icons/hicolor/${size}x${size}/apps/521c.png"
done
# Root icon used by appimagetool for the file icon.
install -m 0644 "$ASSETS/icons/521c_256.png" "$APPDIR/521c.png"

# 3. appimagetool -------------------------------------------------------------
TOOL="${APPIMAGETOOL:-}"
if [ -z "$TOOL" ]; then
    TOOL="$(command -v appimagetool || true)"
fi
if [ -z "$TOOL" ] && [ -x "$HOME/.cache/521c-tools/appimagetool-x86_64.AppImage" ]; then
    TOOL="$HOME/.cache/521c-tools/appimagetool-x86_64.AppImage"
fi
if [ -z "$TOOL" ]; then
    cat >&2 <<'MSG'
appimagetool not found. Install it from the official AppImage releases:
  https://github.com/AppImage/appimagetool/releases
then re-run, or point $APPIMAGETOOL at the binary.
MSG
    exit 1
fi

mkdir -p "$DIST"
OUT="$DIST/521C-${VERSION}-x86_64.AppImage"
echo "==> appimagetool -> $OUT"
(cd "$NATIVE" && ARCH=x86_64 "$TOOL" "$APPDIR" "$OUT")

echo "==> done: $OUT"
