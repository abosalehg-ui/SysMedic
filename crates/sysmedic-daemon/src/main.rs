//! sysmedic-fix-helper — the one privileged component of SysMedic.
//!
//! The GUI and CLI never run as root. When the user confirms a fix they
//! spawn this helper through **pkexec**, which asks polkit to authorize the
//! action `io.github.abosalehg_ui.sysmedic.run-fix` (see
//! `data/io.github.abosalehg_ui.sysmedic.policy`). Only after polkit grants
//! it does this binary run — as root — and do the work.
//!
//! Trust boundary: the helper accepts only a fix **id** from the caller,
//! never a command or a plan. It rebuilds the snapshot itself with root
//! privileges and asks the fix registry for the plan, so a compromised
//! unprivileged caller cannot smuggle in arbitrary commands.
//!
//! Usage:
//!   sysmedic-fix-helper apply <fix-id>
//!   sysmedic-fix-helper undo
//!   sysmedic-fix-helper list-journal

use std::process::ExitCode;

use sysmedic_core::Engine;
use sysmedic_fixes::{
    apply, journal_path, plan, undo, CommandRunner, Journal, RealRunner, SYSTEM_JOURNAL,
};

fn snapshot() -> sysmedic_core::Snapshot {
    // Reuse the checkup engine purely for collection (no diagnostics needed).
    let report = Engine::new()
        .with_collectors(sysmedic_collectors::default_collectors())
        .run();
    report.snapshot
}

fn open_journal() -> Result<Journal, String> {
    // The helper always runs as root, so this resolves to SYSTEM_JOURNAL.
    Journal::load(journal_path())
}

fn run() -> Result<String, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !sysmedic_fixes::is_root() {
        return Err(
            "sysmedic-fix-helper must run as root; it is meant to be launched via pkexec".into(),
        );
    }
    let runner: &dyn CommandRunner = &RealRunner;
    match args.as_slice() {
        [cmd, fix_id] if cmd == "apply" => {
            let snapshot = snapshot();
            let plan = plan(fix_id, &snapshot)
                .ok_or_else(|| format!("fix '{fix_id}' is unknown or not applicable right now"))?;
            let mut journal = open_journal()?;
            let outcome = apply(&plan, runner, &mut journal)?;
            Ok(format!(
                "Applied {} ({}). Journal: {SYSTEM_JOURNAL}",
                outcome.fix_id, plan.title
            ))
        }
        [cmd] if cmd == "undo" => {
            let mut journal = open_journal()?;
            let title = undo(runner, &mut journal)?;
            Ok(format!("Reverted: {title}"))
        }
        [cmd] if cmd == "list-journal" => {
            let journal = open_journal()?;
            let mut out = String::new();
            for e in journal.entries() {
                out.push_str(&format!(
                    "{}  {}  {}{}\n",
                    e.applied_at,
                    e.fix_id,
                    e.title,
                    if e.undone { "  (undone)" } else { "" }
                ));
            }
            Ok(out)
        }
        _ => Err("usage: sysmedic-fix-helper <apply <fix-id>|undo|list-journal>".into()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("sysmedic-fix-helper: {error}");
            ExitCode::FAILURE
        }
    }
}
