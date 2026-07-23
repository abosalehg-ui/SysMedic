# Architecture

SysMedic follows Clean Architecture: a pure domain core, infrastructure at the
edges, and presentation layers (CLI, GUI) that only talk to the application
engine.

```
┌──────────────────────────────────────────────────────────┐
│ Presentation   sysmedic-gui  (GTK4/libadwaita, MVVM, M2) │
│                sysmedic-cli  (clap)                      │
├──────────────────────────────────────────────────────────┤
│ Application    Engine: run collectors → snapshot →       │
│                diagnostics → findings → health score     │
├──────────────────────────────────────────────────────────┤
│ Domain         sysmedic-core: Snapshot, Finding,         │
│ (no I/O)       Severity, Category, HealthReport,         │
│                traits Collector / Diagnostic / Fixer     │
├──────────────────────────────────────────────────────────┤
│ Infrastructure sysmedic-collectors (procfs, sysfs,       │
│                systemd, dpkg/apt, snap, sshd, ufw)       │
│                sysmedic-knowledge (embedded YAML, en/ar) │
│                sysmedic-fixes, sysmedic-report,          │
│                sysmedic-daemon (D-Bus + polkit, M3)      │
└──────────────────────────────────────────────────────────┘
```

## Why Rust + GTK4/libadwaita

- **Performance & footprint:** a health tool must cost ~nothing at idle; Rust
  has no GC and minimal memory overhead.
- **Safety:** this tool triggers privileged operations — memory safety is not
  a luxury here.
- **Native look:** libadwaita gives authentic GNOME HIG, automatic dark/light.
- **Precedent:** Mission Center proves the exact stack works beautifully.
- **Packaging:** a single binary makes Flatpak/deb/AppImage/Snap all easy.

Python/GTK would be quicker to write but slower and heavier to ship; Go lacks
mature GTK4 bindings. Decision per the project's rule: performance and
maintainability over ease of writing.

## Crate map

| Crate | Layer | Responsibility |
|---|---|---|
| `sysmedic-core` | Domain | Data model, traits, engine, weighted health scoring |
| `sysmedic-collectors` | Infra | Read the system: CPU, memory, disks, thermal, processes, services, packages, boot, logs, network, security, battery, snap |
| `sysmedic-diagnostics` | Infra | Pure rules `fn(&Snapshot) -> Vec<Finding>`; stable finding ids |
| `sysmedic-knowledge` | Infra | Embedded bilingual (en/ar) explanations per finding id; `Explainer` trait for optional LLM backends (M6) |
| `sysmedic-fixes` | Infra | Fix contract + implementations (M3): preview, dry-run, undo journal |
| `sysmedic-report` | Infra | JSON / Markdown / HTML rendering (PDF via HTML print, M5) |
| `sysmedic-daemon` | Infra | `sysmedicd`: privileged D-Bus helper + polkit (M3), scheduler + notifications (M5) |
| `sysmedic-cli` | Presentation | `sysmedic checkup / checks / explain` |
| `sysmedic-gui` | Presentation | GTK4/libadwaita app (M2) |

## Key design decisions

### Checkup flow
`Engine::run()` executes each `Collector` (every section of `Snapshot` is
`Option` — anything unreadable is skipped and noted in `collection_errors`),
then feeds the snapshot through every `Diagnostic`. Findings carry a stable id
(`storage.disk_nearly_full`), severity, category, evidence and an optional fix
hint. The score starts at 100 per category, subtracts a penalty per finding
(Info 0 / Low 5 / Medium 12 / High 25 / Critical 40, floor 0), and the overall
score is the category-weight average (Storage/Security 15, Memory/Services 12,
Thermal/Packages 10, Boot/CPU 8, others 5).

### Testability rule
Collectors separate I/O from parsing: every parser is a pure function tested
against fixture strings (`parse_meminfo`, `parse_blame`, `parse_df`, ...).
Diagnostics are pure and tested against fixture snapshots. A knowledge-base
test asserts every finding id has an English **and** Arabic explanation.

### Privilege model (M3)
The GUI/CLI never run as root. Fixes execute in `sysmedicd`, a small D-Bus
system service authorized per-action via polkit. Every fix must present a
`FixPlan` — description, exact commands, affected paths, reversibility, undo
procedure, risk — before the user can confirm. Applied fixes are recorded in a
transaction journal that powers undo.

### AI Explain
Offline-first: the embedded knowledge base answers the five questions (cause,
dangerous?, impact, remedy, risk-if-ignored) with zero network access. The
`Explainer` trait allows an optional LLM provider (e.g. Claude API) in M6 for
deeper, context-aware explanations — strictly opt-in.
