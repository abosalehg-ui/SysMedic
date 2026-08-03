# Changelog


## [Unreleased]

### Security
- Report and state files are now forced to `0600` **after** opening, not only
  at creation. `OpenOptions::mode` is ignored for a file that already exists,
  so regenerating a report over one an editor had rewritten as `0644` silently
  republished hostnames, listening ports and the package inventory
  world-readable.
- `apply` now journals a fix as `pending` *before* running its commands and
  only then marks it `applied`/`failed`. Journalling afterwards meant a failed
  journal write — most plausibly a full disk, a common reason to run SysMedic
  at all — left the system changed with nothing for `undo` to find.
- `SYSMEDIC_HELPER` is compiled out of release builds. It could redirect
  `pkexec` at an arbitrary binary; not an escalation (polkit still
  authenticates) but the prompt named a program the user had not chosen.
- History is written with `O_NOFOLLOW`, matching the journal. Neither file
  falls back to `/tmp` any more when `HOME`/`XDG_STATE_HOME` are unset — a
  predictable path in a world-traversable directory invited a planted symlink.
- Filenames printed by `sysmedic disk` are stripped of control characters, so
  a crafted name cannot inject terminal escape sequences.
- `smartctl` is invoked with `--` before the device path.
- Replaced the archived `serde_yaml` 0.9 with the maintained `serde_yaml_ng`.

### Added
- An application icon (scalable + symbolic), installed by all four packaging
  formats. The `.desktop` file and About dialog had always named one.
- The disk scan reports progress and can be cancelled, and the Disk Usage page
  can scan any folder rather than only `$HOME`.
- Treemap tiles are keyboard-navigable (arrows, Home/End) with a visible focus
  ring, and announce themselves to assistive technology.
- Reduced-motion support in the app and on both site pages.

### Changed
- Fix plans carry bilingual titles and descriptions, so the Arabic consent
  dialog translates the substance of a privileged change and not just the
  labels around it. Desktop alerts are localized too.
- HTML reports ship an Arabic font stack and isolate evidence blocks to LTR so
  paths and commands are not bidi-reordered inside an RTL document.
- Renamed `packages.security_updates` to `security.updates_pending` and
  `snap.old_revisions` to `storage.snap_old_revisions` so every id prefix
  matches its scoring category. **The old ids still resolve** in
  `sysmedic explain`.
- `FIX_IDS` is derived from the fix registry instead of hand-maintained.

### Fixed
- Rule count in the README, both site pages, `ISSUES.md` and `ROADMAP.md`
  (said 27 and 21; there are 28), now guarded by a test.
- `padding-inline-*` instead of physical properties on the Arabic page.

All notable changes to SysMedic are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] — 2026-07-26

The hardening-and-honesty release: everything from the comprehensive
engineering/UX/security review, the complete Arabic story, and package
hygiene beyond Debian.

### Added
- **dnf security advisories** — `security_upgrades` is populated from
  `dnf updateinfo --list --security`, with a dnf-appropriate remedy.
- **pacman cache finding** — `packages.pacman_cache_large` (with bilingual
  knowledge and a `paccache -rk2` hint); pacman never prunes its cache.
- **Real Arabic screenshot** — `dashboard-ar.png` is now captured from the
  live app running under an `ar_SA.UTF-8` locale, confirming full RTL
  mirroring end-to-end.
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
