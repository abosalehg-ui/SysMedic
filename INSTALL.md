# Installing SysMedic

SysMedic runs on any modern Linux; Ubuntu/Debian gets the fullest coverage.
Anything unavailable (no battery in a VM, no systemd in a container) is skipped
gracefully and reported as a skipped check.

## From a `.deb` (Ubuntu/Debian) — recommended

Download `sysmedic_<version>_amd64.deb` from the
[Releases](https://github.com/abosalehg-ui/SysMedic/releases) page, then:

```bash
sudo apt install ./sysmedic_*.deb
sysmedic checkup        # CLI
sysmedic-gui            # desktop app
```

This installs the CLI, the desktop app, the polkit-authorized fix helper and the
`.desktop`/AppStream/polkit integration.

## From source

Requires Rust stable. The GUI additionally needs GTK4/libadwaita dev libraries;
the CLI builds without them.

```bash
# Build deps for the GUI (skip for CLI-only):
sudo apt install libgtk-4-dev libadwaita-1-dev

git clone https://github.com/abosalehg-ui/SysMedic
cd SysMedic

cargo run --release -p sysmedic-cli -- checkup   # CLI
cargo run --release -p sysmedic-gui              # GUI
```

To install the CLI to your `PATH`:

```bash
cargo install --path crates/sysmedic-cli
```

## Other package formats

Flatpak, AppImage and Snap manifests live in [`packaging/`](packaging/README.md)
with build instructions for each.

> **Auto-Fix requires the `.deb`.** The polkit action authorizes one exact
> path, `/usr/libexec/sysmedic-fix-helper`, which only the `.deb` installs on
> the host. Under Flatpak, Snap and AppImage the helper lives inside the
> bundle, so applying fixes is unavailable there. Diagnostics — the read-only
> checkup, explanations, disk analysis and reports — work in every format.

## Optional: deep explanations

The offline knowledge base explains every finding with **no network access**.
If you want deeper, machine-specific explanations you can bring your own Claude
API key — it is strictly opt-in:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
sysmedic explain storage.disk_nearly_full --deep
# optionally pick a model:
export SYSMEDIC_LLM_MODEL=claude-opus-4-8
```

Without the key, `--deep` prints the offline answer and a hint. Only the finding
id and its evidence are ever sent — never files or credentials.

## Optional runtime helpers

- **`smartmontools`** — richer SMART disk-health findings (`sudo apt install smartmontools`).
- **`chromium` / `wkhtmltopdf`** — PDF export (`sysmedic checkup --format pdf`);
  falls back to HTML if neither is present.

## Environment variables

Every one is optional; SysMedic works with none of them set.

| Variable | Effect |
|---|---|
| `ANTHROPIC_API_KEY` | Enables `explain --deep`. Absent, `--deep` prints the offline answer and a hint. Only the finding id and its evidence are ever sent. |
| `SYSMEDIC_LLM_MODEL` | Overrides the Claude model used by `--deep`. |
| `XDG_STATE_HOME` | Where the health-score history and the per-user fix journal live. Falls back to `$HOME/.local/state`. If neither is set, SysMedic reports that it cannot record history rather than writing to a shared temp directory. |
| `LANG` | Selects the explanation, report and fix-consent language (`ar*` → Arabic, otherwise English). Override per-command with `--lang`. |
| `NO_COLOR` | Suppresses ANSI color in CLI output. Color is also dropped automatically when writing to a file or a pipe. |
| `SYSMEDIC_HELPER` | **Debug builds only.** Points `pkexec` at an alternative fix-helper binary for development. Compiled out of release builds so nothing can redirect the privileged prompt. |

## Scheduling checkups

```bash
sysmedic schedule daily     # systemd user timer — survives reboots, no idle cost
sysmedic history            # health-score trend
sysmedic schedule off       # stop
```
