//! Fix implementations for SysMedic.
//!
//! SysMedic never executes a fix silently. The flow is always:
//! 1. [`plan`] — build a [`FixPlan`] from a snapshot (pure, no side effects)
//! 2. the caller shows [`FixPlan::preview`] and gets explicit confirmation
//! 3. [`apply`] — run the plan's commands through a [`CommandRunner`] and
//!    record the transaction in the [`Journal`]
//! 4. [`undo`] — reverse the most recent reversible transaction
//!
//! Steps 3–4 require root and run inside the privileged `sysmedic-fix-helper`
//! (invoked via polkit/pkexec); the GUI and CLI never run as root themselves.

pub mod command;
pub mod fixes;
pub mod journal;

use std::path::{Path, PathBuf};

pub use command::{CommandRunner, RealRunner, RecordingRunner};
pub use fixes::{fix_for_finding, Fix, FIX_IDS};
pub use journal::{Journal, JournalEntry};
use sysmedic_core::fix::FixPlan;
use sysmedic_core::Snapshot;

/// System-wide journal path used by the privileged helper.
pub const SYSTEM_JOURNAL: &str = "/var/lib/sysmedic/journal.json";

/// Outcome of applying a fix.
#[derive(Debug)]
pub struct ApplyOutcome {
    pub fix_id: String,
    pub outputs: Vec<String>,
}

/// Build the plan for `fix_id` against `snapshot`, or `None` if the fix is
/// unknown or not applicable.
pub fn plan(fix_id: &str, snapshot: &Snapshot) -> Option<FixPlan> {
    fixes::find(fix_id)?.plan(snapshot)
}

/// Every fix that currently applies to `snapshot`, as ready-to-preview plans.
pub fn applicable_plans(snapshot: &Snapshot) -> Vec<FixPlan> {
    fixes::all()
        .iter()
        .filter_map(|f| f.plan(snapshot))
        .collect()
}

/// Run a plan's commands through `runner` and record the transaction.
///
/// Stops at the first failing command (a fix is not left half-applied
/// silently — the error names the command that failed). Callers must have
/// obtained user confirmation before calling this.
pub fn apply(
    plan: &FixPlan,
    runner: &dyn CommandRunner,
    journal: &mut Journal,
) -> Result<ApplyOutcome, String> {
    let mut outputs = Vec::new();
    for command in &plan.commands {
        outputs.push(runner.run(command)?);
    }
    journal.record(JournalEntry {
        fix_id: plan.id.clone(),
        title: plan.title.clone(),
        applied_at: journal::now_rfc3339(),
        reversible: plan.reversible,
        undo: plan.undo.clone(),
        undone: false,
    })?;
    Ok(ApplyOutcome {
        fix_id: plan.id.clone(),
        outputs,
    })
}

/// Undo the most recent reversible transaction in `journal`.
pub fn undo(runner: &dyn CommandRunner, journal: &mut Journal) -> Result<String, String> {
    let (index, entry) = journal
        .last_undoable()
        .ok_or("nothing to undo — no reversible fix has been applied")?;
    let (title, undo_commands) = (entry.title.clone(), entry.undo.clone());
    for command in &undo_commands {
        runner.run(command)?;
    }
    journal.mark_undone(index)?;
    Ok(title)
}

/// Where the CLI should read/write the journal: the system path when running
/// as root, otherwise a per-user state file.
pub fn journal_path() -> PathBuf {
    if is_root() {
        return PathBuf::from(SYSTEM_JOURNAL);
    }
    let base = std::env::var("XDG_STATE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| Path::new(&h).join(".local/state"))
        })
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join("sysmedic/journal.json")
}

pub fn is_root() -> bool {
    // Safe: geteuid never fails and has no preconditions.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysmedic_core::snapshot::SecurityInfo;

    fn ufw_disabled() -> Snapshot {
        Snapshot {
            security: Some(SecurityInfo {
                firewall_active: Some(false),
                ssh_permit_root_login: None,
                ssh_password_auth: None,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn apply_runs_commands_and_records() {
        let snapshot = ufw_disabled();
        let plan = plan("fix.enable_ufw", &snapshot).unwrap();
        let runner = RecordingRunner::new();
        let dir = tempfile::tempdir().unwrap();
        let mut journal = Journal::load(dir.path().join("j.json")).unwrap();

        let outcome = apply(&plan, &runner, &mut journal).unwrap();
        assert_eq!(outcome.fix_id, "fix.enable_ufw");
        assert_eq!(runner.commands()[0].display(), "ufw --force enable");
        assert_eq!(journal.entries().len(), 1);
    }

    #[test]
    fn apply_then_undo_runs_the_undo_command() {
        let snapshot = ufw_disabled();
        let plan = plan("fix.enable_ufw", &snapshot).unwrap();
        let runner = RecordingRunner::new();
        let dir = tempfile::tempdir().unwrap();
        let mut journal = Journal::load(dir.path().join("j.json")).unwrap();

        apply(&plan, &runner, &mut journal).unwrap();
        let title = undo(&runner, &mut journal).unwrap();
        assert_eq!(title, "Enable the firewall");
        let commands = runner.commands();
        assert_eq!(commands.last().unwrap().display(), "ufw disable");
        // Second undo finds nothing left.
        assert!(undo(&runner, &mut journal).is_err());
    }

    #[test]
    fn a_failing_command_aborts_and_is_not_recorded() {
        let snapshot = ufw_disabled();
        let plan = plan("fix.enable_ufw", &snapshot).unwrap();
        let runner = RecordingRunner::failing_on("ufw");
        let dir = tempfile::tempdir().unwrap();
        let mut journal = Journal::load(dir.path().join("j.json")).unwrap();

        assert!(apply(&plan, &runner, &mut journal).is_err());
        assert!(journal.entries().is_empty());
    }
}
