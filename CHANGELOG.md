# Changelog

All notable changes to SysMedic are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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
- **Engine & CLI (M1)** — weighted 0–100 health score; 15 collectors (CPU, memory,
  disks, thermal, processes, services, packages, boot, logs, network, security,
  battery, snap, flatpak, SMART, ports); 21+ diagnostic rules with stable ids;
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
