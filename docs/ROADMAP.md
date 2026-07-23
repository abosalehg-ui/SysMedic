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

## M2 — Desktop app (GTK4/libadwaita)
- Application shell, dashboard with health-score gauge and category bars
- Findings list with severity badges; explanation pane (the five questions)
- Run checkup from the UI; dark/light via libadwaita; ar/en localization
- MVVM: composite templates + view models over the async engine API

## M3 — Auto Fix, safely
- `sysmedicd` D-Bus system helper + polkit policy (GUI never runs as root)
- Fix preview dialog: what happens / commands / affected files / reversible?
- Undo journal and `sysmedic undo`
- First fixes: apt clean, journal vacuum, autoremove old kernels, snap
  retain=2, flatpak remove unused, disable service, enable ufw

## M4 — Advanced tools
- Disk analyzer with treemap/sunburst visualization
- Network panel: per-process usage, open ports, DNS, latency
- Security audit: firewall, SSH hardening, open ports, security updates,
  risky services, weak configs
- SMART collector (smartctl JSON) + disk-health diagnostics

## M5 — Follow-up
- Scheduler: daily/weekly/monthly checkups via systemd user timers
- Notifications: disk full, overheating, low RAM, security updates
- HTML report polish + PDF export; health-score history

## M6 — 1.0 release
- Packaging: Flatpak (primary/Flathub), deb, AppImage, Snap
- Optional LLM `Explainer` provider (opt-in, bring your own key)
- Full ar/en localization pass, website, screenshots
