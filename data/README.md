# System integration files

These are installed by packaging (Flatpak/deb/AppImage; see milestone M6). For
a manual install on a Debian/Ubuntu system:

| File | Destination | Purpose |
|---|---|---|
| `io.github.abosalehg_ui.SysMedic.desktop` | `/usr/share/applications/` | App launcher entry |
| `io.github.abosalehg_ui.SysMedic.metainfo.xml` | `/usr/share/metainfo/` | AppStream metadata (software centers) |
| `io.github.abosalehg_ui.sysmedic.policy` | `/usr/share/polkit-1/actions/` | polkit actions authorizing the two fix helpers |

Both privileged helper binaries must be installed to the paths the polkit
policy names and owned by root:

```sh
install -Dm755 target/release/sysmedic-fix-helper /usr/libexec/sysmedic-fix-helper
install -Dm755 target/release/sysmedic-fix-helper-destructive \
    /usr/libexec/sysmedic-fix-helper-destructive
install -Dm644 data/io.github.abosalehg_ui.sysmedic.policy \
    /usr/share/polkit-1/actions/io.github.abosalehg_ui.sysmedic.policy
install -Dm644 data/io.github.abosalehg_ui.SysMedic.desktop \
    /usr/share/applications/io.github.abosalehg_ui.SysMedic.desktop
install -Dm644 data/io.github.abosalehg_ui.SysMedic.metainfo.xml \
    /usr/share/metainfo/io.github.abosalehg_ui.SysMedic.metainfo.xml
```

## Privilege model

The GUI and CLI **never run as root**. When the user confirms a fix, they
launch `pkexec /usr/libexec/sysmedic-fix-helper[-destructive] apply <fix-id>`.
pkexec asks polkit to authorize the matching action (admin authentication by
default). Only then does the helper run as root.

**Two helpers, two actions.** pkexec derives the polkit action from the path of
the program it launches, so one binary could only ever produce one prompt — the
same generic wording for emptying a download cache and for purging packages
with their configuration. SysMedic therefore ships two:

| Binary | polkit action | Fixes |
|---|---|---|
| `sysmedic-fix-helper` | `…sysmedic.run-fix` | reversible setting changes (`fix.enable_ufw`, `fix.snap_retain`) |
| `sysmedic-fix-helper-destructive` | `…sysmedic.run-fix-destructive` | changes `undo` cannot reverse (`fix.apt_clean`, `fix.journal_vacuum`, `fix.autoremove`, `fix.flatpak_unused`) |

Each binary accepts only the ids of its own class and refuses the other's, so
the split is enforced in code rather than merely described in the prompt — and
an administrator can allow the routine class without the destructive one.

A helper accepts a fix **id** only — never a command or a plan. It rebuilds the
system snapshot itself and asks the fix registry for the plan, so an
unprivileged caller cannot inject arbitrary commands. Every applied fix is
recorded in `/var/lib/sysmedic/journal.json`, which powers `sysmedic undo`.
