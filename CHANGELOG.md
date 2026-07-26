# Changelog

All notable changes to SysMedic are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Finding titles in Arabic** — the finding titles/summaries themselves now
  localize through a per-id message catalog in the knowledge base (templates
  with `{0}`-style placeholders filled from the finding's recorded values),
  completing the Arabic story across the CLI report, HTML/Markdown reports
  and the GUI. Missing translations fall back to the English rule text.
- **Export from the GUI** — "Export report…" (Ctrl+E) saves the last checkup
  as HTML, Markdown or JSON via a file dialog, written owner-only (0600).
- **dnf and pacman support** — the packages collector detects the system's
  package manager and reports correct upgradable/broken counts on Fedora and
  Arch (with manager-appropriate fix hints) instead of a "dpkg not found"
  error; apt keeps the fullest coverage.

### Security
- polkit: `allow_active` is `auth_admin` (was `auth_admin_keep`) — a cached
  authorization let any session process silently re-invoke the fix helper
  (e.g. `undo` right after enabling the firewall) for ~5 minutes.
- CI/supply chain: least-privilege `GITHUB_TOKEN`, every GitHub Action pinned
  to a commit SHA, and a `cargo audit` (RUSTSEC) gate on `Cargo.lock`.
- `notify-send` gets `--` before dynamic text; C0/C1 control characters are
  stripped from finding titles/evidence and LLM output before terminal
  rendering; PDF converters no longer run with `--no-sandbox`; reports and
  history are written `0600`; the scheduled systemd user unit is sandboxed
  (`NoNewPrivileges`, `PrivateTmp`, `ProtectSystem=full`).

### Changed
- **Parallel checkup** — collectors run on scoped threads; the checkup now
  takes about as long as its slowest tool instead of the sum of ~16 of them.
- **Arabic** — category labels, grades, severity badges, the CLI report
  chrome, the HTML report (including `lang="ar"`) and, most importantly, the
  fix-confirmation preview are now localized; `Lang` moved into
  `sysmedic-core`.
- **GUI** — adaptive narrow-width layout (`adw::Breakpoint` +
  `ViewSwitcherBar`), toast feedback while and after a fix runs, destructive
  styling for irreversible fixes, a monospace start-aligned consent preview,
  local-time timestamps, LevelBar color offsets, F5/Ctrl+R refresh, disk-scan
  error/empty states, a curated treemap palette with contrast-aware labels
  and an accessible list view of the same data.
- **CLI** — every command honors `NO_COLOR` and pipes (not just `checkup`);
  checkup output prints each finding's id so `sysmedic explain` is
  discoverable.
- Thresholds for boot, journal, battery, SMART and packages are centralized
  in `sysmedic_core::thresholds` and shared with fix applicability; engine
  construction has a single composition root
  (`sysmedic_diagnostics::default_engine`).
- The `sysmedic-daemon` crate is renamed `sysmedic-fix-helper` to match what
  it actually is; typed errors (`FixError`/`JournalError`/`HistoryError`)
  replace stringly errors; systemd durations with day/week units parse; the
  Flatpak unused-runtime check matches refs exactly.

### Added
- **Optional LLM deep explanations** — `sysmedic explain <id> --deep` asks Claude
  for a context-aware explanation on top of the offline knowledge base. Strictly
  opt-in via `ANTHROPIC_API_KEY` (model overridable with `SYSMEDIC_LLM_MODEL`);
  it sends only the finding id and its evidence, and falls back silently to the
  offline answer on any error. Implemented over the raw Messages API behind an
  injectable HTTP transport, so request-building and response-parsing are
  unit-tested without a network.
- **Packaging** — `.deb` build script (verified), plus Flatpak, AppImage and
  Snap manifests under `packaging/`.
- **Release CI** — a tag-triggered workflow builds the `.deb` and attaches it to
  the GitHub release.
- Project `INSTALL.md`, a GitHub Pages website page under `docs/site/`, and this
  changelog.

## [0.1.0] — M1–M5

The full doctor's visit: **checkup → diagnose → explain → prescribe → follow-up.**

### Added
- **Engine & CLI (M1)** — weighted 0–100 health score; 16 collectors (CPU, memory,
  disks, thermal, processes, services, packages, boot, logs, network, security,
  battery, snap, flatpak, SMART, ports); 27 diagnostic rules with stable ids;
  bilingual (en/ar) offline knowledge base; `checkup` (text/json/markdown/html),
  `checks`, `explain`.
- **Desktop app (M2)** — GTK4/libadwaita GUI (MVVM), automatic dark/light, Arabic
  and English, dashboard with score hero and per-category bars, findings list
  with the five-question explanation pane, checkup on a worker thread.
- **Safe fixes (M3)** — preview → apply → undo with a transaction journal; six
  fixes (apt clean, journal vacuum, autoremove kernels, snap retain, flatpak
  remove unused, enable ufw); privileged `sysmedic-fix-helper` authorized through
  pkexec/polkit — the app never runs as root; the helper accepts a fix id only
  and rebuilds the plan, so nothing can be injected.
- **Advanced tools (M4)** — disk analyzer with a squarified treemap (GUI) and a
  CLI breakdown; SMART disk-health diagnostics; security audit of exposed ports,
  SSH password/root login and firewall; `sysmedic network`.
- **Follow-up (M5)** — scheduled checkups via systemd user timers; desktop
  notifications with alert thresholds; append-only health-score history with a
  GUI trend strip; PDF export via a headless browser (HTML fallback).

[Unreleased]: https://github.com/abosalehg-ui/SysMedic/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/abosalehg-ui/SysMedic/releases/tag/v0.1.0
