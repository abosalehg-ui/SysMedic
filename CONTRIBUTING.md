# Contributing to SysMedic

Thanks for helping build the doctor for Linux systems!

## Getting started

```bash
git clone https://github.com/abosalehg-ui/SysMedic
cd SysMedic
cargo build --workspace
cargo test --workspace
cargo run -p sysmedic-cli -- checkup
```

Rust stable is the only requirement for the engine and CLI. The GUI crate
(M2+) will additionally need `libgtk-4-dev libadwaita-1-dev`.

## Ground rules

- `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
  must pass; CI enforces both.
- **Collectors** separate I/O from parsing: parsing is a pure function with
  fixture tests. Collectors never panic and never fail a checkup — record
  problems in `Snapshot::collection_errors`.
- **Diagnostics** are pure (`fn(&Snapshot) -> Vec<Finding>`) and unit-tested.
  Every new finding id must be added to `FINDING_IDS` **and** get an English +
  Arabic entry in `crates/sysmedic-knowledge/data/knowledge.yaml` — a test
  fails otherwise.
- **Fixes** (M3+) must declare a complete `FixPlan` (commands, affected paths,
  reversibility, undo, risk) and never execute without user confirmation.
- Keep commits focused; describe *why* in the message body.

## Where to help

See [docs/ISSUES.md](docs/ISSUES.md) for the backlog and
[docs/ROADMAP.md](docs/ROADMAP.md) for milestones. Good first issues: new
diagnostic rules with knowledge-base entries, and collector parsers for more
system facets.
