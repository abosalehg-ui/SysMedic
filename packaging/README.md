# Packaging SysMedic

Four package formats, each self-contained in this directory. All build from a
release compile of the workspace.

| Format | File | Needs | Status |
|---|---|---|---|
| Debian/Ubuntu `.deb` | [`deb/build-deb.sh`](deb/build-deb.sh) | `dpkg-deb` | ✅ builds & verified in CI |
| Flatpak | [`flatpak/io.github.abosalehg_ui.SysMedic.yaml`](flatpak/io.github.abosalehg_ui.SysMedic.yaml) | `flatpak-builder`, GNOME SDK | manifest ready; Flathub submission is manual |
| AppImage | [`appimage/build-appimage.sh`](appimage/build-appimage.sh) | `appimagetool` (GTK plugin for full GUI bundling) | script ready |
| Snap | [`snap/snapcraft.yaml`](snap/snapcraft.yaml) | `snapcraft` | strict-confinement manifest; not yet built/verified |

## `.deb` (recommended for Ubuntu/Debian)

```bash
cargo build --release -p sysmedic-cli -p sysmedic-daemon -p sysmedic-gui
packaging/deb/build-deb.sh          # -> sysmedic_<version>_<arch>.deb
sudo apt install ./sysmedic_*.deb
```

The GUI is packaged only if `target/release/sysmedic-gui` exists (it needs
`libgtk-4-dev libadwaita-1-dev` to compile); otherwise the package is CLI-only.
The build installs the binary, the polkit-authorized fix helper into
`/usr/libexec`, and the `.desktop`, AppStream and polkit assets.

## Flatpak

```bash
flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47 \
                        org.freedesktop.Sdk.Extension.rust-stable//24.08
cd packaging/flatpak
flatpak-builder --user --install --force-clean build \
  io.github.abosalehg_ui.SysMedic.yaml
flatpak run io.github.abosalehg_ui.SysMedic
```

The manifest uses `--share=network` during build so it can fetch crates. A
**Flathub** submission must instead be reproducible/offline: generate a
`cargo-sources.json` with
[`flatpak-cargo-generator`](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo)
(`python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json`), add it
to the manifest `sources:` and drop the `--share=network` build-arg. Submitting
to Flathub then means opening a PR against `flathub/flathub` — an external step
that needs a Flathub account.

## AppImage

```bash
cargo build --release -p sysmedic-cli -p sysmedic-daemon -p sysmedic-gui
packaging/appimage/build-appimage.sh   # -> SysMedic-<version>-<arch>.AppImage
```

The script assembles the AppDir and calls `appimagetool` when present. For a
fully portable GUI, run it under
[`linuxdeploy`](https://github.com/linuxdeploy/linuxdeploy) with its GTK plugin
so the GTK4/libadwaita runtime is bundled.

## Snap

```bash
cd packaging/snap && snapcraft
sudo snap install ./sysmedic_*.snap --dangerous
```

The manifest uses **strict** confinement with the `gnome` extension (which
provides the GTK4/libadwaita runtime) and the `system-observe` /
`hardware-observe` / `mount-observe` / `log-observe` / `network-observe`
interfaces for diagnostics. Snapcraft rejects extensions under `classic`
confinement, so classic is not an option while the GUI needs the gnome
extension. Some deep diagnostics that need unmediated host access are best run
from the `.deb`.

## Note on privileged fixes

The **`.deb`** installs `sysmedic-fix-helper` to `/usr/libexec` and the polkit
policy to `/usr/share/polkit-1/actions`, so Auto-Fix works: fixes are
authorized per-action through polkit and the GUI/CLI never run as root.

Under **Flatpak, Snap, and AppImage** the polkit action's `exec.path` points at
the host path `/usr/libexec/sysmedic-fix-helper`, which those sandboxed/bundled
formats do not install on the host — so **Auto-Fix currently requires the
`.deb`**. Making fixes work from a sandbox needs a host-side privileged service
(e.g. a systemd system service activated over D-Bus) that the sandboxed app
calls; that is not yet implemented. Diagnostics (the read-only checkup) work in
every format.
