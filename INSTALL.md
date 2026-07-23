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

## Scheduling checkups

```bash
sysmedic schedule daily     # systemd user timer — survives reboots, no idle cost
sysmedic history            # health-score trend
sysmedic schedule off       # stop
```
