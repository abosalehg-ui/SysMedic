# Issue backlog

The GitHub issue list, grouped by milestone. Items marked ✅ shipped with the
initial M0/M1 increment; the rest are open work.

## M0 — Foundation
1. ✅ Scaffold cargo workspace (9 crates, Clean Architecture layout)
2. ✅ CI: fmt + clippy (-D warnings) + tests + release build + smoke run
3. ✅ Docs: vision, competitive analysis, architecture, roadmap
4. ✅ License (GPL-3.0) + contributing guide

## M1 — Engine + CLI MVP
5. ✅ Core model: Snapshot, Finding, Severity, Category, FixPlan contract
6. ✅ Weighted health-score engine + grades
7. ✅ Collectors: CPU/loadavg, meminfo, df, thermal (thermal_zone + hwmon)
8. ✅ Collectors: processes (zombies, top RSS), systemd services, boot
   (systemd-analyze time/blame)
9. ✅ Collectors: packages (dpkg audit, old kernels, apt cache, upgradable/
   security), journal & /var/log sizes, network (route/DNS), security
   (ufw, sshd PermitRootLogin), battery health, snap revisions
10. ✅ 21 diagnostic rules with stable finding ids + fixture tests
11. ✅ Bilingual knowledge base (en/ar) + coverage test
12. ✅ CLI: `checkup` (text/json/markdown/html, --output, --lang), `checks`,
    `explain`
13. Collector: GPU (vendor detection, VRAM, driver presence)
14. Collector: SMART via `smartctl --json` (needs polkit or udisks2 in GUI)
15. Diagnostic: missing drivers (ubuntu-drivers devices)
16. Diagnostic: orphan packages (deborphan-style reverse-dependency scan)
17. Diagnostic: flatpak unused runtimes
18. Diagnostic: filesystem errors in journal (EXT4-fs error, I/O error)
19. Diagnostic: permissions problems (world-writable system dirs, broken
    home ownership)

## M2 — GUI
20. ✅ GTK4/libadwaita application shell + .desktop file + AppStream metainfo
    (app icon asset still pending)
21. ✅ Dashboard: score hero, category level bars, "Run checkup" flow on a
    worker thread
22. ✅ Findings list: severity badges, explanation pane (5 questions),
    evidence, suggested command (filters pending)
23. ar/en localization: built-in strings shipped; gettext migration + full
    RTL audit pending
24. ✅ CI: GTK dev dependencies installed in the workflow

## M3 — Auto Fix
25. ✅ Privileged fix helper (`sysmedic-fix-helper`) authorized via
    polkit/pkexec + polkit policy; GUI/CLI never run as root
    (resident D-Bus `sysmedicd` moved to M5 with the scheduler)
26. ✅ FixPlan preview + confirmation flow (CLI `--dry-run`, GUI dialog)
27. ✅ Transaction journal + `sysmedic undo`
28. ✅ Fixes: apt clean, journal vacuum, autoremove kernels, snap retain,
    flatpak remove unused, enable ufw (parameterized `disable service`
    deferred — it needs a target-picker UI)
29. ✅ Fix-engine + journal unit tests; live apply/undo verified as root
30. ✅ Flatpak collector + diagnostic + bilingual knowledge entry

## M4 — Advanced tools
31. Disk analyzer backend (parallel du) + treemap widget
32. Network panel: per-process traffic, open ports, latency, DNS info
33. Security audit page (aggregate + explain)

## M5 — Follow-up
34. `sysmedicd` resident D-Bus service hosting the scheduler
35. Scheduler via systemd user timers (daily/weekly/monthly)
36. Desktop notifications (disk, thermal, RAM, security updates)
37. Health history storage + trend chart; PDF export

## M6 — 1.0
38. Flatpak manifest + Flathub submission; deb/AppImage/Snap packaging
39. Optional LLM Explainer provider (opt-in API key)
40. Website + screenshots + release announcement
