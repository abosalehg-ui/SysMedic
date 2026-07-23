# Roadmap

## M0 — Foundation ✅
Workspace scaffold, docs (vision, competitive analysis, architecture, roadmap,
issues), CI (fmt + clippy + tests + build), GPL-3.0.

## M1 — Engine + CLI MVP ✅
- `sysmedic-core`: model, engine, weighted health score
- 13 collectors: CPU, memory, disks, thermal, processes, services, packages,
  boot, logs, network, security, battery, snap
- 21 diagnostic rules with stable ids
- Bilingual (en/ar) offline knowledge base
- `sysmedic checkup` (text/JSON/Markdown/HTML), `checks`, `explain`
- Unit tests for every parser and rule; CI green

## M2 — Desktop app (GTK4/libadwaita) ✅
- Application shell (AdwToolbarView + HeaderBar), health-score hero with
  color-coded grade, category rows with level bars
- Findings list of expander rows: severity badges, the five explanation
  questions per finding, evidence, suggested command
- Checkup runs on a worker thread (`gio::spawn_blocking`) — the UI never
  blocks; a refresh button re-runs it
- Dark/light automatic via libadwaita; ar/en strings built in (full gettext
  in M6); desktop entry + AppStream metainfo in `data/`
- MVVM-lite: all presentation logic in a pure, unit-tested `viewmodel` module

## M3 — Auto Fix, safely ✅
- Pure, unit-tested fix engine (`sysmedic-fixes`): plan → preview → apply →
  undo, with a `CommandRunner` seam so tests never touch the system
- 6 fixes: apt clean, journal vacuum, autoremove kernels, snap retain=2,
  flatpak remove unused, enable ufw — each with structured commands,
  affected paths, risk and (where possible) a reversible undo
- Privilege model via **polkit/pkexec**: the GUI/CLI never run as root; on
  confirmation they launch `sysmedic-fix-helper` through pkexec, which
  authorizes the `io.github.abosalehg_ui.sysmedic.run-fix` action. The helper
  accepts a fix **id** only and rebuilds the plan itself, so no command can be
  injected by an unprivileged caller
- Transaction journal at `/var/lib/sysmedic/journal.json` + `sysmedic undo`
- Fix preview everywhere (CLI `--dry-run`, GUI confirmation dialog): what
  happens / commands / affected paths / reversible?
- Also added: flatpak collector, diagnostic and knowledge entry
- (A resident D-Bus `sysmedicd` service is deferred to M5, where the
  scheduler actually needs a long-running process; on-demand fixes need only
  the pkexec helper.)

## M4 — Advanced tools ✅
- Disk analyzer: `sysmedic-diskscan` crate (directory size tree + pure,
  unit-tested squarified-treemap layout); GUI Disk Usage page renders the
  treemap with cairo on a worker thread; CLI `sysmedic disk [path]`
- SMART collector (`smartctl --json`) + disk-health diagnostics (failing
  self-assessment, reallocated sectors, SSD wear) + bilingual knowledge
- Security audit: listening-port collector (`/proc/net/tcp{,6}`, no external
  tool) flagging services exposed beyond localhost; SSH `PasswordAuthentication`
  hardening check; these join the existing firewall/root-login/security-update
  findings in every checkup
- Network view: `sysmedic network` (default route, DNS, listening ports,
  latency); GUI restructured into an AdwViewStack (Overview + Disk Usage)
- (Per-process live bandwidth deferred: it needs sampling and elevated
  privileges; open ports with scope cover the exposed-service question.)

## M5 — Follow-up ✅
- Scheduler: `sysmedic schedule daily|weekly|monthly|off|status` installs a
  **systemd user timer** running `sysmedic monitor` — the Linux-native way to
  schedule work (survives reboots, zero idle cost, battery-friendly). Unit-file
  builders are pure and unit-tested
- Notifications: `sysmedic monitor` evaluates alert thresholds (disk full,
  overheating, low RAM, pending security updates) and fires desktop
  notifications via `notify-send` (degrades if absent). Thresholds are pure/tested
- Health history: `sysmedic-history` crate appends every checkup to a JSONL
  log; `sysmedic history` shows a sparkline + trend; the GUI overview gains a
  trend strip; both CLI and GUI record on each checkup
- PDF export: `sysmedic checkup --format pdf` renders the HTML report through a
  headless browser (chromium) or wkhtmltopdf, falling back to HTML if neither
  is installed
- (A resident D-Bus `sysmedicd` proved unnecessary: systemd timers *are* the
  scheduler, with a far smaller footprint than an always-on daemon.)

## M6 — 1.0 release ✅
- Packaging (`packaging/`): a verified Debian/Ubuntu `.deb` build script, plus
  Flatpak (GNOME runtime), AppImage and Snap manifests. The `.deb` is built and
  smoke-checked in CI; a tag-triggered **release** workflow attaches it to the
  GitHub release
- Optional LLM `Explainer` (`sysmedic explain <id> --deep`): a raw-HTTP Claude
  Messages API provider behind an injectable `HttpTransport` seam, so
  request-building and response-parsing are unit-tested with a fake transport
  and never touch the network. Strictly opt-in via `ANTHROPIC_API_KEY`
  (`SYSMEDIC_LLM_MODEL` overrides the model); it sends only the finding id and
  its evidence, and falls back silently to the offline answer on any error
- Website (`docs/site/`, GitHub Pages workflow), `INSTALL.md`, `CHANGELOG.md`
- (Note: submitting to Flathub and hosting the Pages site are the two remaining
  manual steps — both need external accounts, not code. A full gettext migration
  is tracked as a post-1.0 refinement; the app already ships complete built-in
  en/ar strings and RTL.)
