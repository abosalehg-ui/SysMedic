# System integration files

These are installed by packaging (Flatpak/deb/AppImage; see milestone M6). For
a manual install on a Debian/Ubuntu system:

| File | Destination | Purpose |
|---|---|---|
| `io.github.abosalehg_ui.SysMedic.desktop` | `/usr/share/applications/` | App launcher entry |
| `io.github.abosalehg_ui.SysMedic.metainfo.xml` | `/usr/share/metainfo/` | AppStream metadata (software centers) |
| `io.github.abosalehg_ui.sysmedic.policy` | `/usr/share/polkit-1/actions/` | polkit action authorizing the fix helper |

The privileged helper binary must be installed to the path the polkit policy
names and owned by root:

```sh
install -Dm755 target/release/sysmedic-fix-helper /usr/libexec/sysmedic-fix-helper
install -Dm644 data/io.github.abosalehg_ui.sysmedic.policy \
    /usr/share/polkit-1/actions/io.github.abosalehg_ui.sysmedic.policy
install -Dm644 data/io.github.abosalehg_ui.SysMedic.desktop \
    /usr/share/applications/io.github.abosalehg_ui.SysMedic.desktop
install -Dm644 data/io.github.abosalehg_ui.SysMedic.metainfo.xml \
    /usr/share/metainfo/io.github.abosalehg_ui.SysMedic.metainfo.xml
```

## Privilege model

The GUI and CLI **never run as root**. When the user confirms a fix, they
launch `pkexec /usr/libexec/sysmedic-fix-helper apply <fix-id>`. pkexec asks
polkit to authorize the `io.github.abosalehg_ui.sysmedic.run-fix` action
(admin authentication by default). Only then does the helper run as root.

The helper accepts a fix **id** only — never a command or a plan. It rebuilds
the system snapshot itself and asks the fix registry for the plan, so an
unprivileged caller cannot inject arbitrary commands. Every applied fix is
recorded in `/var/lib/sysmedic/journal.json`, which powers `sysmedic undo`.
