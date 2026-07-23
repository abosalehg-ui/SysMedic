#!/usr/bin/env bash
# Build a Debian/Ubuntu .deb for SysMedic.
#
# Produces sysmedic_<version>_<arch>.deb containing the CLI, the polkit-authorized
# fix helper, the GUI (if it was built), the .desktop/AppStream/polkit assets and
# the systemd user timer template. Run from the repository root after:
#
#   cargo build --release -p sysmedic-cli -p sysmedic-daemon [-p sysmedic-gui]
#
# The GUI is optional: it is only packaged when target/release/sysmedic-gui exists
# (it needs libgtk-4/libadwaita to build).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
PKG="sysmedic_${VERSION}_${ARCH}"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
chmod 755 "$STAGE"

echo "Packaging SysMedic $VERSION ($ARCH)"

# --- Binaries -------------------------------------------------------------
install -Dm755 target/release/sysmedic          "$STAGE/usr/bin/sysmedic"
install -Dm755 target/release/sysmedic-fix-helper "$STAGE/usr/libexec/sysmedic-fix-helper"
if [[ -x target/release/sysmedic-gui ]]; then
  install -Dm755 target/release/sysmedic-gui    "$STAGE/usr/bin/sysmedic-gui"
else
  echo "note: target/release/sysmedic-gui not found — packaging CLI only"
fi

# --- Desktop integration --------------------------------------------------
install -Dm644 data/io.github.abosalehg_ui.SysMedic.desktop \
  "$STAGE/usr/share/applications/io.github.abosalehg_ui.SysMedic.desktop"
install -Dm644 data/io.github.abosalehg_ui.SysMedic.metainfo.xml \
  "$STAGE/usr/share/metainfo/io.github.abosalehg_ui.SysMedic.metainfo.xml"
install -Dm644 data/io.github.abosalehg_ui.sysmedic.policy \
  "$STAGE/usr/share/polkit-1/actions/io.github.abosalehg_ui.sysmedic.policy"

# --- Docs & licence -------------------------------------------------------
install -Dm644 LICENSE "$STAGE/usr/share/doc/sysmedic/copyright"
install -Dm644 README.md "$STAGE/usr/share/doc/sysmedic/README.md"

# --- Control metadata -----------------------------------------------------
INSTALLED_KB="$(du -ks "$STAGE" | cut -f1)"
mkdir -p "$STAGE/DEBIAN"
cat > "$STAGE/DEBIAN/control" <<EOF
Package: sysmedic
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Depends: libc6, policykit-1 | polkit
Recommends: libgtk-4-1, libadwaita-1-0, smartmontools
Suggests: chromium | chromium-browser | wkhtmltopdf
Installed-Size: $INSTALLED_KB
Maintainer: abosalehg-ui <ar0.history@gmail.com>
Homepage: https://github.com/abosalehg-ui/SysMedic
Description: A doctor for your Linux system
 SysMedic examines your system, gives it a 0-100 health score, diagnoses
 problems, explains each one in plain language (English and Arabic), and
 prescribes safe, reversible fixes authorized through polkit. It also
 analyzes disk usage, audits security, schedules checkups and exports
 reports. The GUI is GTK4/libadwaita; the CLI covers the same ground.
EOF

dpkg-deb --build --root-owner-group "$STAGE" "$REPO_ROOT/$PKG.deb"
echo "Built $PKG.deb"
