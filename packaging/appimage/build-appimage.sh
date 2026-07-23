#!/usr/bin/env bash
# Build a SysMedic AppImage (GUI + CLI + fix helper in one portable file).
#
# Needs: a release build with the GUI, plus `appimagetool` on PATH (or set
# APPIMAGETOOL to its path). Run from anywhere; paths are resolved from the repo.
#
#   cargo build --release -p sysmedic-cli -p sysmedic-daemon -p sysmedic-gui
#   packaging/appimage/build-appimage.sh
#
# Bundling the GTK4/libadwaita runtime fully is best done with linuxdeploy and
# its GTK plugin; this script assembles the AppDir and app metadata, then calls
# appimagetool. On a host without the GTK plugin the CLI still runs anywhere.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
ARCH="$(uname -m)"
APPDIR="packaging/appimage/AppDir"
APPIMAGETOOL="${APPIMAGETOOL:-appimagetool}"

rm -rf "$APPDIR"
install -Dm755 target/release/sysmedic-gui "$APPDIR/usr/bin/sysmedic-gui"
install -Dm755 target/release/sysmedic     "$APPDIR/usr/bin/sysmedic"
install -Dm755 target/release/sysmedic-fix-helper "$APPDIR/usr/libexec/sysmedic-fix-helper"

install -Dm644 data/io.github.abosalehg_ui.SysMedic.desktop \
  "$APPDIR/usr/share/applications/io.github.abosalehg_ui.SysMedic.desktop"
# AppImage expects the .desktop and an icon at the AppDir root.
cp "$APPDIR/usr/share/applications/io.github.abosalehg_ui.SysMedic.desktop" \
   "$APPDIR/io.github.abosalehg_ui.SysMedic.desktop"

# Minimal placeholder icon (a real PNG icon asset is tracked in issue #20).
ICON="$APPDIR/io.github.abosalehg_ui.SysMedic.svg"
cat > "$ICON" <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
  <rect width="128" height="128" rx="24" fill="#3584e4"/>
  <path d="M64 30v68M30 64h68" stroke="#fff" stroke-width="14" stroke-linecap="round"/>
</svg>
SVG

cat > "$APPDIR/AppRun" <<'SH'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "$0")")"
export PATH="$HERE/usr/bin:$PATH"
exec "$HERE/usr/bin/sysmedic-gui" "$@"
SH
chmod +x "$APPDIR/AppRun"

OUT="SysMedic-$VERSION-$ARCH.AppImage"
if command -v "$APPIMAGETOOL" >/dev/null 2>&1; then
  ARCH="$ARCH" "$APPIMAGETOOL" "$APPDIR" "$REPO_ROOT/$OUT"
  echo "Built $OUT"
else
  echo "AppDir assembled at $APPDIR."
  echo "Install appimagetool (or set APPIMAGETOOL) to produce $OUT."
fi
