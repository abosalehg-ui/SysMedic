# Vision

**SysMedic is a doctor for your Linux system** — not another cleaner, not
another monitor. The goal: the first application a user installs after Ubuntu.

## The doctor metaphor drives everything

| Medical step | SysMedic feature |
|---|---|
| Checkup | One-click full system scan → health score /100 |
| Diagnosis | Findings with severity, category and evidence |
| Explanation | Every finding answers: cause? dangerous? impact? remedy? risk if ignored? |
| Prescription | Safe fixes with mandatory informed consent: what happens, which files change, can it be undone |
| Follow-up | Scheduled checkups, proactive notifications, health history |

## Product principles

1. **Explain, don't just report.** A number without an interpretation is noise.
2. **Consent before treatment.** No fix runs without a preview and an explicit
   confirmation; everything reversible keeps an undo path. The GUI never runs
   as root — privileged actions go through a polkit-authorized D-Bus helper.
3. **Offline-first.** The knowledge base is embedded and bilingual (en/ar).
   An LLM backend is an optional, opt-in enhancement — never a requirement.
4. **Degrade gracefully.** No smartctl? No battery? Inside a container? Skip
   the check, say so, never crash.
5. **GNOME-native and fast.** libadwaita UI, dark/light, Rust core with a
   near-zero idle footprint.
6. **Trust is the product.** A system tool that ever surprises the user has
   failed. Conservative defaults, honest reporting, no dark patterns.

## Target users

- **The switcher** installing Ubuntu for the first time — needs explanations.
- **The daily driver** whose system "got slow" — needs diagnosis, not graphs.
- **The tinkerer** — wants the CLI, JSON output and scriptability.

## Success criteria

- A novice can go from "my laptop is slow" to an applied, safe fix without
  opening a terminal.
- `sysmedic checkup` becomes the standard first command in Ubuntu support
  threads.
- 1.0 ships on Flathub with deb/AppImage/Snap alternatives.
