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

## M4 — Advanced tools
- Disk analyzer with treemap/sunburst visualization
- Network panel: per-process usage, open ports, DNS, latency
- Security audit: firewall, SSH hardening, open ports, security updates,
  risky services, weak configs
- SMART collector (smartctl JSON) + disk-health diagnostics

## M5 — Follow-up
- `sysmedicd` resident service (D-Bus) hosting the scheduler
- Scheduler: daily/weekly/monthly checkups via systemd user timers
- Notifications: disk full, overheating, low RAM, security updates
- HTML report polish + PDF export; health-score history

## M6 — 1.0 release
- Packaging: Flatpak (primary/Flathub), deb, AppImage, Snap
- Optional LLM `Explainer` provider (opt-in, bring your own key)
- Full ar/en localization pass, website, screenshots
