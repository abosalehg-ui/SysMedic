# Changelog


## [Unreleased]

### Security

- **The firewall check can finally fire.** `firewall_active()` could only ever
  return `Some(false)` from `ufw status`, which needs root — and both the
  `security.firewall_inactive` rule and `fix.enable_ufw` require exactly that
  value. On the normal unprivileged run (the mode the whole app is built
  around) a machine with ufw installed and switched off produced no finding, no
  fix button, and a clean 100 for Security. It now reads `ENABLED=` from
  `/etc/ufw/ufw.conf`, which is world-readable, and falls back to firewalld's
  unit state only when firewalld is actually installed.
- **New rule `security.index_stale`.** The pending-security-update count is
  computed from the package index, so on a machine that has not refreshed in
  months "0 security updates pending" was an absence of knowledge reported as
  an all-clear. The index age is now collected (apt, dnf and pacman) and a
  Medium finding is raised past seven days.
- **UDP sockets are audited.** `security.exposed_ports` describes itself as
  "services listening on the network" but only read `/proc/net/tcp{,6}`, so
  mDNS/avahi, a resolver on 53, SSDP and WireGuard were invisible. `/proc/net/udp{,6}`
  is now parsed too.
- **Two privileged helpers, two polkit actions.** pkexec derives its action
  from the path of the program it launches, so one binary meant one generic
  prompt for both "empty the download cache" and "purge packages and their
  configuration". `sysmedic-fix-helper` now handles reversible setting changes
  and `sysmedic-fix-helper-destructive` the rest; each refuses the other's fix
  ids, and an administrator can allow one class without the other.
- **The PDF converter runs under the sanitized `PATH`, and with a timeout.**
  It was the one external command that inherited the ambient `PATH`, and the
  only one with no time limit — a wedged headless browser hung the CLI
  indefinitely. Tool detection no longer launches each candidate browser just
  to ask its version, and the output path is made absolute so it cannot be
  read as an option.

### Fixed

- **A mounted ISO no longer tanks the health score.** `iso9660`, `udf` and
  `erofs` are read-only images that are 100% full by construction; they were
  raising a Critical `storage.disk_nearly_full` finding, and a Critical caps
  the overall score at 59. Plugging in an install USB dropped the machine from
  "Excellent" to "Poor".
- **The score no longer counts what was never measured.** A category with no
  data produced no findings and therefore scored 100, so a container without
  systemd, battery or SMART graded higher than a laptop with one full disk.
  The weighted average now covers measured categories only, and the report,
  dashboard and JSON carry a `coverage` figure ("9/12") next to the score.
- **`undo` for `fix.snap_retain` restores the user's own value.** It hardcoded
  snapd's default of 3, so someone running `refresh.retain=10` who applied the
  fix and pressed undo silently ended up at 3 — which makes "reversible: yes"
  in the consent dialog untrue. The previous value is captured in the plan,
  carried in the journal, and validated as an integer in snapd's range before
  a root process substitutes it.
- **The `fix.autoremove` preview names what it actually removes.** It spoke of
  old kernels while `autoremove --purge` deletes every auto-installed package
  nothing needs any more, along with their `/etc` configuration. The plan now
  lists the packages (from an unprivileged `apt-get -s` simulation) and lists
  `/etc` among the affected paths.
- **Severity badges are localized.** The GUI and the terminal report printed
  the machine-facing `CRITICAL` above Arabic text; they now use
  `Severity::label_in`, which already existed and was already used by the HTML
  report.
- **Values interpolated into Arabic text are bidi-isolated.** Mount points,
  device nodes and unit names are Latin/neutral runs inside RTL sentences, so
  the bidi algorithm reordered them on screen — a finding title could show its
  path scrambled. `render_template` now wraps each value in FSI/PDI, which
  fixes the GUI, terminal, HTML and Markdown at once.
- **The GUI pins its text direction.** The app chose Arabic strings from the
  environment while GTK derived layout direction from its own translations; if
  those were missing (slim container, Flatpak without locale data) the result
  was Arabic text in a left-to-right layout.
- **The disk page scans lazily.** Building the window kicked off a full walk of
  `$HOME` on every launch, even for a user who never opened the Disk Usage
  tab — minutes of I/O and battery on a large home directory. It now scans the
  first time the page is actually shown.
- **The treemap mirrors in RTL.** It is drawn by hand with cairo, so GTK's
  automatic mirroring never reached it: in an Arabic window the tiles stayed
  left-to-right and the arrow keys walked against the visual order.
- **Language detection follows POSIX.** All five entry points read `LANG`
  alone; `LC_ALL` and `LC_MESSAGES` now take precedence, from one shared
  `Lang::from_env()` instead of five copies of the same line.
- **`sysmedic explain --lang ar` prints Arabic labels.** The five answers were
  localized but their field names were hardcoded English.

### Added

- `sysmedic checkup --exit-code` exits 1 when the worst finding is High and 2
  when it is Critical, so a scheduled checkup can drive monitoring. Without the
  flag the exit code stays 0, as scripts have always been able to rely on.
- End-to-end tests for the `sysmedic` binary (`crates/sysmedic-cli/tests/cli.rs`):
  the `--format json` shape, the exit-code contract, refusal of unknown finding
  and fix ids, and bilingual `explain` output. Everything else in the workspace
  is a unit test over a pure function; nothing covered the interface people
  actually script against.
- The GUI string tables are checked exhaustively (every field, both languages)
  by destructuring `Strings`, so a newly added string cannot ship untranslated.

### Changed

- The default deep-explanation model is `claude-opus-5`. The previous id was
  stale and would be rejected, which made `--deep` fail silently back to the
  offline answer. `max_tokens` was also raised from 1024, which current models
  can spend on reasoning before the answer begins, and the request asks for low
  effort since restating a diagnosed finding is not a hard reasoning task.
- Arabic prose in the knowledge base uses the Arabic percent sign `٪`
  consistently; the file mixed it with `%` inside the same paragraph. The
  policy is documented at the top of `knowledge.yaml`.
- `CODE_REVIEW_SysMedic_2026-08-03.md` moved to `docs/`, alongside the other
  reviews.

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
