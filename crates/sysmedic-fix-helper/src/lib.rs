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
//! **Two binaries, two polkit actions.** pkexec chooses its polkit action from
//! the path of the program it launches, so one binary can only ever map to one
//! action — and one action means one generic prompt for both "empty the
//! download cache" and "purge packages and their configuration files". This
//! crate therefore builds two thin binaries over the same logic:
//!
//! * `sysmedic-fix-helper` — [`FixTier::Routine`] fixes: undoable setting
//!   changes.
//! * `sysmedic-fix-helper-destructive` — [`FixTier::Destructive`] fixes: things
//!   `undo` cannot bring back.
//!
//! Each accepts only the ids of its own tier, so the split is enforced in code
//! and not merely described in the prompt.
//!
//! Usage:
//!   sysmedic-fix-helper[-destructive] apply <fix-id>
//!   sysmedic-fix-helper[-destructive] undo
//!   sysmedic-fix-helper[-destructive] list-journal

use std::process::ExitCode;

use sysmedic_fixes::{
    apply, fix_ids_for, journal_path, plan, undo, CommandRunner, FixTier, Journal, RealRunner,
    SYSTEM_JOURNAL,
};

fn snapshot() -> sysmedic_core::Snapshot {
    // Collection only — the helper needs no diagnostics to rebuild a plan.
    sysmedic_collectors::default_snapshot()
}

fn open_journal() -> Result<Journal, String> {
    // The helper always runs as root, so this resolves to SYSTEM_JOURNAL and
    // is never `None` — but handle it rather than unwrapping in a root process.
    let path = journal_path().ok_or_else(|| "no journal path available".to_string())?;
    Journal::load(path).map_err(|e| e.to_string())
}

/// The helper reports in the language of the caller's environment, so the GUI
/// and CLI can surface its message unchanged.
fn helper_lang() -> sysmedic_core::Lang {
    sysmedic_core::Lang::from_env()
}

/// What the helper was asked to do. The only free-form input from the caller
/// is a fix **id**, and it must be one of the compiled-in ids — everything
/// else is rejected here, before any privileged work, so a compromised caller
/// cannot smuggle in a command or an unexpected verb.
#[derive(Debug, PartialEq)]
pub enum Action {
    Apply(String),
    Undo,
    ListJournal,
}

const USAGE: &str = "usage: sysmedic-fix-helper <apply <fix-id>|undo|list-journal>";

/// Validate the caller's request for a helper of `tier`.
///
/// An id belonging to the *other* tier is rejected here, before any privileged
/// work: authorization was granted for this class of change, so this binary
/// must not perform another one.
pub fn parse_action(tier: FixTier, args: &[String]) -> Result<Action, String> {
    match args {
        [cmd, fix_id] if cmd == "apply" => {
            if !fix_ids_for(tier).contains(&fix_id.as_str()) {
                return Err(match sysmedic_fixes::tier_of(fix_id) {
                    Some(other) => {
                        format!("fix '{fix_id}' is {other:?}; this helper only runs {tier:?} fixes")
                    }
                    None => format!("unknown fix id '{fix_id}'"),
                });
            }
            Ok(Action::Apply(fix_id.clone()))
        }
        [cmd] if cmd == "undo" => Ok(Action::Undo),
        [cmd] if cmd == "list-journal" => Ok(Action::ListJournal),
        _ => Err(USAGE.into()),
    }
}

fn run(tier: FixTier) -> Result<String, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !sysmedic_fixes::is_root() {
        return Err(
            "sysmedic-fix-helper must run as root; it is meant to be launched via pkexec".into(),
        );
    }
    let runner: &dyn CommandRunner = &RealRunner;
    match parse_action(tier, &args)? {
        Action::Apply(fix_id) => {
            let snapshot = snapshot();
            let plan = plan(&fix_id, &snapshot)
                .ok_or_else(|| format!("fix '{fix_id}' is unknown or not applicable right now"))?;
            let mut journal = open_journal()?;
            let outcome = apply(&plan, runner, &mut journal).map_err(|e| e.to_string())?;
            Ok(format!(
                "Applied {} ({}). Journal: {SYSTEM_JOURNAL}",
                outcome.fix_id,
                plan.title_in(helper_lang())
            ))
        }
        Action::Undo => {
            let mut journal = open_journal()?;
            // The entry to reverse is only known once the journal is read, so
            // the tier check happens here rather than in `parse_action`: this
            // binary was authorized for one class of change and must not
            // perform another, whatever the journal happens to hold.
            if let Some((_, entry)) = journal.last_undoable() {
                match sysmedic_fixes::tier_of(&entry.fix_id) {
                    Some(entry_tier) if entry_tier != tier => {
                        return Err(format!(
                            "the last undoable fix '{}' is {entry_tier:?}; run the {entry_tier:?} \
                             helper instead",
                            entry.fix_id
                        ));
                    }
                    None => {
                        return Err(format!("cannot undo unknown fix '{}'", entry.fix_id));
                    }
                    _ => {}
                }
            }
            let title = undo(runner, &mut journal, helper_lang()).map_err(|e| e.to_string())?;
            Ok(format!("Reverted: {title}"))
        }
        Action::ListJournal => {
            let journal = open_journal()?;
            let mut out = String::new();
            for e in journal.entries() {
                out.push_str(&format!(
                    "{}  {}  {}  {}{}\n",
                    e.applied_at,
                    e.fix_id,
                    // Surface the state so a Pending/Failed entry — a fix that
                    // may have got partway — is visible in the audit listing.
                    match e.state {
                        sysmedic_fixes::EntryState::Applied => "applied",
                        sysmedic_fixes::EntryState::Pending => "pending",
                        sysmedic_fixes::EntryState::Failed => "failed",
                    },
                    e.title,
                    if e.undone { "  (undone)" } else { "" }
                ));
            }
            Ok(out)
        }
    }
}

/// Entry point shared by both binaries.
pub fn main_for(tier: FixTier) -> ExitCode {
    match run(tier) {
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
            parse_action(FixTier::Routine, &args(&["apply", "fix.enable_ufw"])),
            Ok(Action::Apply("fix.enable_ufw".into()))
        );
        // The id is the only free-form input, and it is validated against the
        // compiled-in registry: anything else is rejected before any root work.
        for hostile in [
            "fix.enable_ufw; rm -rf /",
            "../../etc/passwd",
            "fix.nonexistent",
        ] {
            assert!(parse_action(FixTier::Routine, &args(&["apply", hostile])).is_err());
            assert!(parse_action(FixTier::Destructive, &args(&["apply", hostile])).is_err());
        }
    }

    #[test]
    fn a_helper_refuses_fixes_from_the_other_tier() {
        // polkit authorized *this* class of change. Running the other class
        // under that authorization would make the prompt a lie, so each binary
        // rejects the other tier's ids outright.
        let err = parse_action(FixTier::Routine, &args(&["apply", "fix.autoremove"]))
            .expect_err("routine helper must refuse a destructive fix");
        assert!(err.contains("only runs"), "unhelpful error: {err}");

        let err = parse_action(FixTier::Destructive, &args(&["apply", "fix.enable_ufw"]))
            .expect_err("destructive helper must refuse a routine fix");
        assert!(err.contains("only runs"), "unhelpful error: {err}");

        // Each accepts its own.
        assert!(parse_action(FixTier::Destructive, &args(&["apply", "fix.autoremove"])).is_ok());
        assert!(parse_action(FixTier::Routine, &args(&["apply", "fix.enable_ufw"])).is_ok());
    }

    #[test]
    fn apply_requires_exactly_one_id() {
        assert!(parse_action(FixTier::Routine, &args(&["apply"])).is_err());
        assert!(parse_action(
            FixTier::Routine,
            &args(&["apply", "fix.enable_ufw", "extra"])
        )
        .is_err());
    }

    #[test]
    fn undo_and_list_take_no_arguments() {
        assert_eq!(
            parse_action(FixTier::Routine, &args(&["undo"])),
            Ok(Action::Undo)
        );
        assert_eq!(
            parse_action(FixTier::Routine, &args(&["list-journal"])),
            Ok(Action::ListJournal)
        );
        assert!(parse_action(FixTier::Routine, &args(&["undo", "x"])).is_err());
    }

    #[test]
    fn unknown_verbs_are_rejected() {
        assert!(parse_action(FixTier::Routine, &args(&["delete-everything"])).is_err());
        assert!(parse_action(FixTier::Routine, &args(&[])).is_err());
    }
}
