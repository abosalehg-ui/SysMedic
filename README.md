# SysMedic 🩺

**A doctor for your Linux system** — it examines, diagnoses, explains in plain
language, prescribes safe fixes, and follows up. Not another cleaner, not
another monitor: the app you install first on a fresh Ubuntu.

```
  Health score: 97/100  (Excellent)

  Storage    ███████░░░  76
  Security   ██████████ 100
  ...

  [MEDIUM] Filesystem / is 88% full
      Only 29.6 GiB free of 252.0 GiB on /.
      Remedy: Clean the APT cache, vacuum the journal, remove old snaps...
      Try: sudo apt clean && sudo journalctl --vacuum-size=200M
```

## Why SysMedic?

Every existing tool is a monitor (htop, btop, Mission Center), a cleaner
(BleachBit, Stacer) or an expert CLI (systemd-analyze, smartctl, journalctl).
**Nobody diagnoses, explains and fixes safely.** SysMedic does all five steps
of a doctor's visit:

| Step | What you get |
|---|---|
| **Checkup** | One command scans CPU, RAM, swap, disks, thermal, battery, services, processes, boot, logs, packages, network and security → a 0–100 health score |
| **Diagnose** | 21+ rules: slow boot, full disks, zombie processes, overheating, failed services, broken/old packages, huge logs, snap bloat, DNS issues, SSH root login, inactive firewall... |
| **Explain** | Every finding answers, offline and in English + العربية: what caused it? is it dangerous? what's the impact? how do I fix it? what if I ignore it? |
| **Prescribe** | Safe fix suggestions today; one-click fixes with preview + undo via a polkit helper in M3 — the app never runs as root |
| **Follow-up** | Scheduled checkups and proactive notifications (M5) |

## The desktop app (M2)

A GNOME-native GTK4/libadwaita application — automatic dark/light, Arabic and
English, checkup on a background thread:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev   # build deps
cargo run --release -p sysmedic-gui
```

The dashboard shows the health score and per-category bars; each finding
expands into the five doctor questions (cause / dangerous? / impact / remedy /
risk if ignored) with evidence and a suggested command.

## Try the CLI (M1)

```bash
cargo run --release -p sysmedic-cli -- checkup            # colored report
sysmedic checkup --format json                            # machine-readable
sysmedic checkup --format html --output report.html       # shareable report
sysmedic explain storage.disk_nearly_full --lang ar       # explain any finding
sysmedic checks                                           # list all rules
```

Requires Rust stable; runs on any modern Linux (Ubuntu/Debian gets the fullest
coverage). Anything unavailable — no battery, no systemd in a container — is
skipped gracefully and reported as a skipped check.

## Project

- **Stack:** Rust workspace; GTK4/libadwaita GUI arriving in M2 (GNOME-native,
  dark/light).
- **Architecture:** Clean Architecture — pure domain core, pluggable
  collectors/diagnostics/fixes, presentation on top.
  See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
- **Where it's going:** [docs/ROADMAP.md](docs/ROADMAP.md) ·
  [docs/ISSUES.md](docs/ISSUES.md) ·
  [docs/VISION.md](docs/VISION.md) ·
  [docs/COMPETITIVE-ANALYSIS.md](docs/COMPETITIVE-ANALYSIS.md)
- **Contributing:** [CONTRIBUTING.md](CONTRIBUTING.md)
- **License:** GPL-3.0-or-later

---

## بالعربية

**SysMedic طبيب لنظام لينكس**: يفحص النظام ويعطيه درجة صحية من 100، يشخّص
المشاكل (بطء الإقلاع، امتلاء القرص، ارتفاع الحرارة، الخدمات المتعطلة، الحزم
المكسورة...)، ويشرح كل مشكلة بالعربية دون إنترنت: ما سببها؟ هل هي خطيرة؟ ما
تأثيرها؟ كيف تُصلح؟ وما خطر تجاهلها؟ — مع إصلاحات آمنة قابلة للمعاينة
والتراجع قادمة في المرحلة M3، وواجهة GNOME حديثة في M2.
