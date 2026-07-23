# Competitive Analysis

*Why does SysMedic deserve to exist? Because every existing tool is either a
monitor, a cleaner, or an expert CLI — none of them is a doctor.*

## The landscape

| Tool | Type | Strengths | Weaknesses |
|---|---|---|---|
| **Stacer** (C++/Qt) | All-in-one dashboard | Cleaner, services, startup apps, uninstaller in one place; the closest thing to a "system app" | Effectively **abandoned for years**; Qt look is alien on GNOME; deletes without diagnosis, explanation or risk analysis |
| **BleachBit** (Python/GTK) | Cleaner | Very powerful and granular; root mode | Dated UI; **dangerous for novices** (options that can break the system); deletion only — no diagnosis |
| **Cockpit** (web) | Server admin | Excellent systemd/journal/storage/network management | Server-oriented, runs in a browser; no health score, no cleaning, not a desktop experience |
| **htop / btop** (TUI) | Monitor | Superb real-time monitoring, tiny footprint | Monitoring only — no history, no diagnosis, no fixes; not for non-technical users |
| **Mission Center** (Rust/GTK4) | Monitor | The best-looking GNOME-native resource monitor (CPU/GPU/RAM/net) | **Shows numbers, never interprets them**; fixes nothing |
| **GNOME Usage / Baobab** | Viewer | Simple, preinstalled | Extremely limited: disk/memory display only |
| **ncdu** (TUI) | Disk analyzer | Fast, excellent at its one job | Single-purpose, terminal only |
| **systemd-analyze** | CLI diagnostic | Precise boot analysis (blame, critical-chain) | Expert CLI; raw output with no "so what do I do?" |
| **smartctl** | CLI diagnostic | Complete SMART health data | Output is cryptic to anyone but experts |
| **journalctl** | CLI logs | Every system log | An ocean without a compass for non-experts |
| **apt / snap / flatpak** | Package managers | Powerful package management | Cleanup is scattered (autoremove, snap revisions, unused flatpak runtimes) with no unified view |

## The gaps nobody fills

1. **Nobody diagnoses.** Every tool either displays numbers (monitors) or
   deletes files (cleaners). No tool says: *"Your boot is slow because unit X
   takes 12 seconds, disabling it is safe, here is the button."*
2. **Nobody explains.** smartctl, journalctl and systemd-analyze own the data,
   but nothing translates it into human language: cause / danger / impact /
   remedy / risk-if-ignored.
3. **Nobody fixes safely.** BleachBit deletes with no preview or undo. No tool
   shows "what will happen + can it be undone + which files change" before
   acting.
4. **No unified health score.** Nothing condenses disk, thermals, SMART,
   packages and services into one 0–100 number a human can track over time.
5. **No follow-up.** No scheduled checkups with proactive, GNOME-native
   notifications.

## The opportunity

> **SysMedic = Checkup → Diagnose → Explain → Prescribe (safe fix) → Follow-up**

Mission Center shows you the symptoms. BleachBit sells medicine without a
prescription. SysMedic is the doctor: it examines, explains in plain language,
prescribes a safe treatment with informed consent, and follows up on your
recovery. That position — and being GNOME-native, fast (Rust) and fully
functional offline — is the wedge that can make it *the first app installed
after Ubuntu*.
