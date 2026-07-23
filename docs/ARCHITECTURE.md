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
│ (no I/O)       Severity, Category, HealthReport, FixPlan,│
│                traits Collector / Diagnostic            │
├──────────────────────────────────────────────────────────┤
│ Infrastructure sysmedic-collectors (procfs, sysfs,       │
│                systemd, dpkg/apt, snap, flatpak, ufw)   │
│                sysmedic-knowledge (embedded YAML, en/ar) │
│                sysmedic-fixes (engine + journal),        │
│                sysmedic-report, sysmedic-daemon          │
│                (sysmedic-fix-helper: pkexec + polkit)    │
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
| `sysmedic-core` | Domain | Data model, traits, engine, weighted health scoring, alert thresholds |
| `sysmedic-collectors` | Infra | Read the system: CPU, memory, disks, thermal, processes, services, packages, boot, logs, network, security, battery, snap, flatpak, SMART, ports |
| `sysmedic-diagnostics` | Infra | Pure rules `fn(&Snapshot) -> Vec<Finding>`; stable finding ids |
| `sysmedic-knowledge` | Infra | Embedded bilingual (en/ar) explanations per finding id; `Explainer` trait + opt-in `LlmExplainer` (Claude Messages API over a testable HTTP seam) |
| `sysmedic-fixes` | Infra | Fix engine: plan → preview → apply → undo, with a `CommandRunner` seam and a transaction journal |
| `sysmedic-diskscan` | Infra | Directory size tree + pure squarified-treemap layout |
| `sysmedic-history` | Infra | Append-only health-score history (JSONL) + trend/sparkline |
| `sysmedic-report` | Infra | JSON / Markdown / HTML / PDF (headless-browser print) rendering |
| `sysmedic-daemon` | Infra | `sysmedic-fix-helper`: the pkexec/polkit-authorized privileged fix executor (scheduling uses systemd user timers, not a resident daemon) |
| `sysmedic-cli` | Presentation | `checkup / checks / explain / fix / undo / disk / network / monitor / history / schedule` |
| `sysmedic-gui` | Presentation | GTK4/libadwaita app (Overview + Disk Usage) |

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

### Privilege model
The GUI/CLI never run as root. On confirmation they launch
`sysmedic-fix-helper` through **pkexec**, which asks **polkit** to authorize
the `io.github.abosalehg_ui.sysmedic.run-fix` action; only then does the helper
run as root. The trust boundary is deliberately narrow: the helper accepts a
fix **id** only — never a command or a serialized plan — and rebuilds the plan
itself from a fresh privileged snapshot, so a compromised unprivileged caller
cannot inject commands. Every fix must present a `FixPlan` (description, exact
structured commands, affected paths, reversibility, undo commands, risk) before
the user can confirm; the same plan drives both the preview and the run. Applied
fixes are recorded in a transaction journal (`/var/lib/sysmedic/journal.json`)
that powers undo. The fix engine itself is pure and unit-tested through a
`CommandRunner` seam, so the security-critical logic is verified without
touching the system.

An on-demand pkexec helper (rather than a resident D-Bus daemon) is the right
fit for M3: it has no long-running root surface. The resident `sysmedicd`
service arrives in M5, where the scheduler genuinely needs a persistent process.

### AI Explain
Offline-first: the embedded knowledge base answers the five questions (cause,
dangerous?, impact, remedy, risk-if-ignored) with zero network access. The
`Explainer` trait allows an optional LLM provider for deeper, context-aware
explanations — strictly opt-in. `LlmExplainer` (in `sysmedic-knowledge::llm`)
implements it against the Claude Messages API: `sysmedic explain <id> --deep`
enables it only when `ANTHROPIC_API_KEY` is set (`SYSMEDIC_LLM_MODEL` overrides
the model, default `claude-opus-4-8`). It sends only the finding id and its
evidence text — never files or credentials — and degrades silently to the
offline answer on any error. Rust has no official Anthropic SDK, so it speaks
the API over raw HTTP; the network call sits behind an `HttpTransport` trait, so
request-building and response-parsing are unit-tested with a fake transport
without ever touching the network.

### Packaging & release
`packaging/` holds all four distribution formats, each building from a release
compile: a verified Debian/Ubuntu `.deb` (`deb/build-deb.sh`, exercised in CI),
a Flatpak manifest (GNOME runtime + rust-stable SDK extension), an AppImage
assembly script, and a Snap `snapcraft.yaml`. Every format installs the
`sysmedic-fix-helper` and the polkit policy so the privilege model holds
regardless of how SysMedic was installed. A tag-triggered `release` workflow
builds the `.deb` and attaches it to the GitHub release; a `pages` workflow
publishes the `docs/site/` landing page.
