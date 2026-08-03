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

use std::path::PathBuf;

pub use command::{CommandRunner, RealRunner, RecordingRunner};
pub use fixes::{fix_for_finding, fix_ids, undo_commands, Fix};
pub use journal::{EntryState, Journal, JournalEntry, JournalError};
use sysmedic_core::fix::FixPlan;
use sysmedic_core::Snapshot;

/// System-wide journal path used by the privileged helper.
pub const SYSTEM_JOURNAL: &str = "/var/lib/sysmedic/journal.json";

/// Why applying or undoing a fix failed. Typed so callers can react to the
/// class of failure (retry an I/O hiccup, surface a failed command verbatim,
/// treat a corrupt journal as needing attention) instead of matching strings.
#[derive(Debug, thiserror::Error)]
pub enum FixError {
    /// A fix command failed; the message is the runner's full report
    /// (command, exit status, stderr).
    #[error("{0}")]
    CommandFailed(String),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error("nothing to undo — no reversible fix has been applied")]
    NothingToUndo,
    #[error("cannot undo unknown fix '{0}'")]
    UnknownFix(String),
}

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
///
/// **Write-ahead ordering.** The entry is journalled as `Pending` *before* the
/// first command runs, and only then moved to `Applied` or `Failed`. Running
/// first and journalling afterwards meant that if the journal write failed —
/// most plausibly because the disk is full, which is a common reason to be
/// running SysMedic at all — the fix had already taken effect but `undo` could
/// never find it. Now the worst case is a `Pending` entry for a fix that did
/// complete, which `undo` still handles.
pub fn apply(
    plan: &FixPlan,
    runner: &dyn CommandRunner,
    journal: &mut Journal,
) -> Result<ApplyOutcome, FixError> {
    let index = journal.record_pending(JournalEntry {
        fix_id: plan.id.clone(),
        // The journal keeps the English title: it is a persisted record whose
        // value must not shift with the locale that happened to be active when
        // the fix ran. Display layers localize via `undo_title_in`.
        title: plan.title.en.clone(),
        applied_at: journal::now_rfc3339(),
        reversible: plan.reversible,
        undo: plan.undo.clone(),
        undone: false,
        state: journal::EntryState::Pending,
    })?;

    let mut outputs = Vec::new();
    for command in &plan.commands {
        match runner.run(command) {
            Ok(output) => outputs.push(output),
            Err(e) => {
                // Best-effort: the command failure is the error worth
                // surfacing, not a follow-on journal problem.
                let _ = journal.finish(index, journal::EntryState::Failed);
                return Err(FixError::CommandFailed(e));
            }
        }
    }
    journal.finish(index, journal::EntryState::Applied)?;
    Ok(ApplyOutcome {
        fix_id: plan.id.clone(),
        outputs,
    })
}

/// Undo the most recent reversible transaction in `journal`.
///
/// The commands to run are rebuilt from the compiled-in fix registry keyed on
/// the entry's `fix_id`, **not** taken from the `undo` commands stored in the
/// journal file. This keeps the trust boundary the same as `apply`: even if
/// the journal on disk were tampered with, `undo` can only run the fixed,
/// audited undo commands of a known fix — never arbitrary injected commands.
pub fn undo(
    runner: &dyn CommandRunner,
    journal: &mut Journal,
    lang: sysmedic_core::Lang,
) -> Result<String, FixError> {
    let (index, entry) = journal.last_undoable().ok_or(FixError::NothingToUndo)?;
    let (stored_title, fix_id) = (entry.title.clone(), entry.fix_id.clone());
    let fix = fixes::find(&fix_id).ok_or_else(|| FixError::UnknownFix(fix_id.clone()))?;
    for command in &fix.undo() {
        runner.run(command).map_err(FixError::CommandFailed)?;
    }
    journal.mark_undone(index)?;
    // Name the reverted fix in the user's language, falling back to whatever
    // the journal recorded if the registry somehow has no title for it.
    let title = fix.title().get(lang).to_string();
    Ok(if title.is_empty() {
        stored_title
    } else {
        title
    })
}

/// The bilingual title of the fix an `undo` would revert, for previews.
pub fn undo_title_in(journal: &Journal, lang: sysmedic_core::Lang) -> Option<String> {
    let (_, entry) = journal.last_undoable()?;
    Some(match fixes::find(&entry.fix_id) {
        Some(fix) => fix.title().get(lang).to_string(),
        None => entry.title.clone(),
    })
}

/// Default install path of the privileged helper. It must match the
/// `org.freedesktop.policykit.exec.path` annotation in
/// `data/io.github.abosalehg_ui.sysmedic.policy`, or polkit will fall through
/// to its generic exec action instead of SysMedic's own.
pub const DEFAULT_HELPER: &str = "/usr/libexec/sysmedic-fix-helper";

/// Which binary `pkexec` should be pointed at.
///
/// The `SYSMEDIC_HELPER` override is a development affordance and is compiled
/// out of release builds. Leaving it in meant anything that could set the
/// environment could choose the binary named in the polkit prompt: not a
/// privilege escalation (an unregistered path falls back to polkit's generic
/// exec action, which still authenticates), but the user would be approving a
/// dialog they reasonably believed was SysMedic's.
pub fn helper_path() -> String {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var("SYSMEDIC_HELPER")
        .ok()
        .filter(|p| !p.is_empty())
    {
        return path;
    }
    DEFAULT_HELPER.to_string()
}

/// Where the CLI should read/write the journal: the system path when running
/// as root, otherwise a per-user state file.
///
/// Returns `None` when neither `XDG_STATE_HOME` nor `HOME` is set. The old
/// `/tmp` fallback put the file at a predictable path in a world-traversable
/// directory, where another local user could pre-create the directory and
/// plant a symlink — so refusing is safer than guessing.
pub fn journal_path() -> Option<PathBuf> {
    if is_root() {
        return Some(PathBuf::from(SYSTEM_JOURNAL));
    }
    sysmedic_core::paths::state_dir().map(|base| base.join("sysmedic/journal.json"))
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
        let title = undo(&runner, &mut journal, sysmedic_core::Lang::En).unwrap();
        assert_eq!(title, "Enable the firewall");
        let commands = runner.commands();
        assert_eq!(commands.last().unwrap().display(), "ufw disable");
        // Second undo finds nothing left.
        assert!(undo(&runner, &mut journal, sysmedic_core::Lang::En).is_err());
    }

    #[test]
    fn undo_ignores_tampered_commands_in_the_journal() {
        // Simulate an attacker-tampered journal: a legitimate fix id but a
        // malicious stored `undo` command. `undo` must run the registry's undo
        // (`ufw disable`), never the injected command.
        let runner = RecordingRunner::new();
        let dir = tempfile::tempdir().unwrap();
        let mut journal = Journal::load(dir.path().join("j.json")).unwrap();
        journal
            .record(JournalEntry {
                fix_id: "fix.enable_ufw".into(),
                title: "Enable the firewall".into(),
                applied_at: "2026-07-24T00:00:00Z".into(),
                reversible: true,
                undo: vec![sysmedic_core::fix::FixCommand::new("rm", &["-rf", "/"])],
                undone: false,
                state: EntryState::Applied,
            })
            .unwrap();

        undo(&runner, &mut journal, sysmedic_core::Lang::En).unwrap();
        let commands = runner.commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].display(), "ufw disable");
    }

    #[test]
    fn a_failing_command_aborts_and_is_recorded_as_failed() {
        let snapshot = ufw_disabled();
        let plan = plan("fix.enable_ufw", &snapshot).unwrap();
        let runner = RecordingRunner::failing_on("ufw");
        let dir = tempfile::tempdir().unwrap();
        let mut journal = Journal::load(dir.path().join("j.json")).unwrap();

        assert!(apply(&plan, &runner, &mut journal).is_err());
        // The entry survives, marked Failed: a fix that got partway must stay
        // visible and undoable, not vanish from the record.
        assert_eq!(journal.entries().len(), 1);
        assert_eq!(journal.entries()[0].state, EntryState::Failed);
    }

    #[test]
    fn the_entry_is_journalled_before_any_command_runs() {
        // The core write-ahead guarantee: by the time a command executes, the
        // journal on disk already knows about it. Verified by having the
        // runner read the journal file back mid-apply.
        use std::sync::Mutex;
        struct PeekingRunner {
            path: std::path::PathBuf,
            seen_on_disk: Mutex<Option<String>>,
        }
        impl CommandRunner for PeekingRunner {
            fn run(&self, _: &sysmedic_core::fix::FixCommand) -> Result<String, String> {
                *self.seen_on_disk.lock().unwrap() = std::fs::read_to_string(&self.path).ok();
                Ok(String::new())
            }
        }

        let snapshot = ufw_disabled();
        let plan = plan("fix.enable_ufw", &snapshot).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("j.json");
        let runner = PeekingRunner {
            path: path.clone(),
            seen_on_disk: Mutex::new(None),
        };
        let mut journal = Journal::load(&path).unwrap();

        apply(&plan, &runner, &mut journal).unwrap();

        let during = runner.seen_on_disk.lock().unwrap().clone();
        let during = during.expect("journal file existed while the command ran");
        assert!(
            during.contains("fix.enable_ufw") && during.contains("pending"),
            "the fix was not journalled as pending before it ran: {during}"
        );
        // ...and it is Applied once the commands succeed.
        assert_eq!(journal.entries()[0].state, EntryState::Applied);
    }

    #[test]
    fn a_pending_entry_is_still_undoable() {
        // Simulates a process killed mid-apply: the entry stayed Pending.
        let dir = tempfile::tempdir().unwrap();
        let mut journal = Journal::load(dir.path().join("j.json")).unwrap();
        journal
            .record(JournalEntry {
                fix_id: "fix.enable_ufw".into(),
                title: "Enable the firewall".into(),
                applied_at: "2026-08-03T00:00:00Z".into(),
                reversible: true,
                undo: vec![],
                undone: false,
                state: EntryState::Pending,
            })
            .unwrap();
        let runner = RecordingRunner::new();
        assert!(undo(&runner, &mut journal, sysmedic_core::Lang::En).is_ok());
        assert_eq!(runner.commands()[0].display(), "ufw disable");
    }
}
