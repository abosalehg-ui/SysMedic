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

/// What the helper was asked to do. The only free-form input from the caller
/// is a fix **id**, and it must be one of the compiled-in ids — everything
/// else is rejected here, before any privileged work, so a compromised caller
/// cannot smuggle in a command or an unexpected verb.
#[derive(Debug, PartialEq)]
enum Action {
    Apply(String),
    Undo,
    ListJournal,
}

const USAGE: &str = "usage: sysmedic-fix-helper <apply <fix-id>|undo|list-journal>";

fn parse_action(args: &[String]) -> Result<Action, String> {
    match args {
        [cmd, fix_id] if cmd == "apply" => {
            if !sysmedic_fixes::FIX_IDS.contains(&fix_id.as_str()) {
                return Err(format!("unknown fix id '{fix_id}'"));
            }
            Ok(Action::Apply(fix_id.clone()))
        }
        [cmd] if cmd == "undo" => Ok(Action::Undo),
        [cmd] if cmd == "list-journal" => Ok(Action::ListJournal),
        _ => Err(USAGE.into()),
    }
}

fn run() -> Result<String, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !sysmedic_fixes::is_root() {
        return Err(
            "sysmedic-fix-helper must run as root; it is meant to be launched via pkexec".into(),
        );
    }
    let runner: &dyn CommandRunner = &RealRunner;
    match parse_action(&args)? {
        Action::Apply(fix_id) => {
            let snapshot = snapshot();
            let plan = plan(&fix_id, &snapshot)
                .ok_or_else(|| format!("fix '{fix_id}' is unknown or not applicable right now"))?;
            let mut journal = open_journal()?;
            let outcome = apply(&plan, runner, &mut journal)?;
            Ok(format!(
                "Applied {} ({}). Journal: {SYSTEM_JOURNAL}",
                outcome.fix_id, plan.title
            ))
        }
        Action::Undo => {
            let mut journal = open_journal()?;
            let title = undo(runner, &mut journal)?;
            Ok(format!("Reverted: {title}"))
        }
        Action::ListJournal => {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn apply_accepts_only_known_fix_ids() {
        assert_eq!(
            parse_action(&args(&["apply", "fix.enable_ufw"])),
            Ok(Action::Apply("fix.enable_ufw".into()))
        );
        // The id is the only free-form input, and it is validated against the
        // compiled-in registry: anything else is rejected before any root work.
        assert!(parse_action(&args(&["apply", "fix.enable_ufw; rm -rf /"])).is_err());
        assert!(parse_action(&args(&["apply", "../../etc/passwd"])).is_err());
        assert!(parse_action(&args(&["apply", "fix.nonexistent"])).is_err());
    }

    #[test]
    fn apply_requires_exactly_one_id() {
        assert!(parse_action(&args(&["apply"])).is_err());
        assert!(parse_action(&args(&["apply", "fix.enable_ufw", "extra"])).is_err());
    }

    #[test]
    fn undo_and_list_take_no_arguments() {
        assert_eq!(parse_action(&args(&["undo"])), Ok(Action::Undo));
        assert_eq!(
            parse_action(&args(&["list-journal"])),
            Ok(Action::ListJournal)
        );
        assert!(parse_action(&args(&["undo", "x"])).is_err());
    }

    #[test]
    fn unknown_verbs_are_rejected() {
        assert!(parse_action(&args(&["delete-everything"])).is_err());
        assert!(parse_action(&args(&[])).is_err());
    }
}
