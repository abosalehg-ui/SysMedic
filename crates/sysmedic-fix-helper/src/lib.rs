//! sysmedic-fix-helper — the one privileged component of SysMedic.
//!
//! The GUI and CLI never run as root. When the user confirms a fix they
//! spawn this helper through **pkexec**, which asks polkit to authorize the
//! action `io.github.abosalehg_ui.sysmedic.run-fix` (see
//! `data/io.github.abosalehg_ui.sysmedic.policy`). Only after polkit grants
//! it does this binary run — as root — and do the work.
//!
//! Trust boundary: the helper accepts only a fix **id** and the
//! **fingerprint** of the plan the user consented to, never a command or a
//! plan. It rebuilds the snapshot itself with root privileges, asks the fix
//! registry for the plan, and refuses if that plan's fingerprint is not the
//! one it was given — so a compromised unprivileged caller cannot smuggle in
//! arbitrary commands, and a user cannot be shown one plan and have another
//! run.
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
//!   sysmedic-fix-helper[-destructive] apply <fix-id> <fingerprint> [--lang=en|ar]
//!   sysmedic-fix-helper[-destructive] undo [--lang=en|ar]
//!   sysmedic-fix-helper[-destructive] list-journal [--lang=en|ar]

use std::process::ExitCode;

use sysmedic_core::fix::FixPlan;
use sysmedic_core::Lang;
#[cfg(test)]
use sysmedic_fixes::helper_lang_arg as lang_arg;
use sysmedic_fixes::{
    apply, fix_ids_for, journal_path, plan, undo, CommandRunner, FixTier, Journal, RealRunner,
    HELPER_LANG_FLAG as LANG_FLAG, SYSTEM_JOURNAL,
};

/// Collect **only** what `fix_id` needs, as root.
///
/// The helper used to call `default_snapshot()`, which runs all sixteen
/// collectors: `smartctl` opening every block device, apt/dpkg/snap/flatpak
/// parsing data that came off the network, `systemd-analyze`, a walk of
/// `/var/log`. All of it as root, all of it to apply one fix that reads one
/// section — and all of it between the user's password and anything visible
/// happening. Each fix declares what its plan reads (`Fix::needs_collectors`),
/// and this builds the snapshot from that alone.
///
/// Collection only — the helper needs no diagnostics to rebuild a plan.
fn snapshot_for(fix_id: &str) -> sysmedic_core::Snapshot {
    let needed = sysmedic_fixes::collectors_for(fix_id).unwrap_or(&[]);
    let collectors: Vec<Box<dyn sysmedic_core::Collector>> =
        sysmedic_collectors::default_collectors()
            .into_iter()
            .filter(|c| needed.contains(&c.name()))
            .collect();
    sysmedic_core::Engine::new()
        .with_collectors(collectors)
        .run()
        .snapshot
}

fn open_journal() -> Result<Journal, String> {
    // The helper always runs as root, so this resolves to SYSTEM_JOURNAL and
    // is never `None` — but handle it rather than unwrapping in a root process.
    let path = journal_path().ok_or_else(|| "no journal path available".to_string())?;
    Journal::load(path).map_err(|e| e.to_string())
}

/// What the helper was asked to do. The only free-form inputs from the caller
/// are a fix **id** and a plan **fingerprint**, and both must have exactly the
/// expected shape (a compiled-in id; 64 lowercase hex digits) — everything
/// else is rejected here, before any privileged work, so a compromised caller
/// cannot smuggle in a command or an unexpected verb.
#[derive(Debug, PartialEq)]
pub enum Action {
    /// Apply the fix with this id, provided the plan rebuilt here has this
    /// fingerprint.
    Apply {
        fix_id: String,
        fingerprint: String,
    },
    Undo,
    ListJournal,
}

/// A parsed invocation: the action, and the language the caller asked the
/// helper to answer in.
///
/// The language travels as an argument rather than through the environment
/// because pkexec hands the helper a minimal environment and `--lang` on the
/// CLI never reached it — an Arabic `sysmedic fix … --yes` ended with an
/// English "Applied …" line from the helper. The value is validated against
/// the two supported tags, so it widens the input surface by one enum.
#[derive(Debug, PartialEq)]
pub struct Request {
    pub action: Action,
    pub lang: Option<Lang>,
}

const USAGE: &str =
    "usage: sysmedic-fix-helper <apply <fix-id> <fingerprint>|undo|list-journal> [--lang=en|ar]";

/// Split a trailing `--lang=<tag>` off `args`, validating the tag.
fn split_lang(args: &[String]) -> Result<(&[String], Option<Lang>), String> {
    match args.last() {
        Some(last) if last.starts_with(LANG_FLAG) => {
            let tag = &last[LANG_FLAG.len()..];
            let lang = Lang::from_tag(tag).ok_or_else(|| format!("unknown language '{tag}'"))?;
            Ok((&args[..args.len() - 1], Some(lang)))
        }
        _ => Ok((args, None)),
    }
}

/// Validate the caller's request for a helper of `tier`.
///
/// An id belonging to the *other* tier is rejected here, before any privileged
/// work: authorization was granted for this class of change, so this binary
/// must not perform another one.
pub fn parse_request(tier: FixTier, args: &[String]) -> Result<Request, String> {
    let (args, lang) = split_lang(args)?;
    let action = match args {
        [cmd, fix_id, fingerprint] if cmd == "apply" => {
            if !fix_ids_for(tier).contains(&fix_id.as_str()) {
                return Err(match sysmedic_fixes::tier_of(fix_id) {
                    Some(other) => {
                        format!("fix '{fix_id}' is {other:?}; this helper only runs {tier:?} fixes")
                    }
                    None => format!("unknown fix id '{fix_id}'"),
                });
            }
            if !FixPlan::is_fingerprint(fingerprint) {
                return Err("the plan fingerprint is malformed".into());
            }
            Action::Apply {
                fix_id: fix_id.clone(),
                fingerprint: fingerprint.clone(),
            }
        }
        [cmd] if cmd == "undo" => Action::Undo,
        [cmd] if cmd == "list-journal" => Action::ListJournal,
        _ => return Err(USAGE.into()),
    };
    Ok(Request { action, lang })
}

/// What to tell the caller when the plan rebuilt as root is not the one the
/// user consented to.
fn plan_changed_message(lang: Lang) -> &'static str {
    match lang {
        Lang::En => {
            "the system changed since the preview, so the fix would no longer do exactly \
             what you approved; run the checkup again and review the new preview"
        }
        Lang::Ar => {
            "تغيّر النظام منذ المعاينة، فلم يعد الإصلاح يفعل بالضبط ما وافقت عليه؛ \
             أعد الفحص وراجع المعاينة الجديدة"
        }
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
    let request = parse_request(tier, &args)?;
    // The helper reports in the language the caller asked for, else the
    // language of its environment, so the GUI and CLI can surface its message
    // unchanged.
    let lang = request.lang.unwrap_or_else(Lang::from_env);
    match request.action {
        Action::Apply {
            fix_id,
            fingerprint,
        } => {
            let snapshot = snapshot_for(&fix_id);
            let plan = plan(&fix_id, &snapshot)
                .ok_or_else(|| format!("fix '{fix_id}' is unknown or not applicable right now"))?;
            // The consent check: what the user read must be what runs.
            if plan.fingerprint() != fingerprint {
                return Err(plan_changed_message(lang).to_string());
            }
            let mut journal = open_journal()?;
            let outcome = apply(&plan, runner, &mut journal).map_err(|e| e.to_string())?;
            Ok(format!(
                "Applied {} ({}). Journal: {SYSTEM_JOURNAL}",
                outcome.fix_id,
                plan.title_in(lang)
            ))
        }
        Action::Undo => {
            let mut journal = open_journal()?;
            // The entry to reverse is only known once the journal is read, so
            // the tier check happens here rather than in `parse_request`: this
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
            let title = undo(runner, &mut journal, lang).map_err(|e| e.to_string())?;
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

    #[test]
    fn fix_collector_names_are_real() {
        // `needs_collectors` is a list of strings matched against
        // `Collector::name()`. A typo there would silently collect nothing,
        // the plan would be built from an empty snapshot, and the fix would
        // report "not applicable right now" — after the user authenticated.
        let known: Vec<&str> = sysmedic_collectors::default_collectors()
            .iter()
            .map(|c| c.name())
            .collect();
        for fix_id in sysmedic_fixes::fix_ids() {
            let needed = sysmedic_fixes::collectors_for(fix_id).expect("known fix id");
            assert!(!needed.is_empty(), "{fix_id} declares no collectors");
            for name in needed {
                assert!(
                    known.contains(name),
                    "{fix_id} needs collector {name:?}, which does not exist"
                );
            }
        }
        assert!(sysmedic_fixes::collectors_for("fix.nope").is_none());
    }

    #[test]
    fn the_helper_collects_less_than_everything() {
        // The point of the narrowing: no fix may quietly pull the whole set
        // back in and run it as root.
        let all = sysmedic_collectors::default_collectors().len();
        for fix_id in sysmedic_fixes::fix_ids() {
            let needed = sysmedic_fixes::collectors_for(fix_id).unwrap().len();
            assert!(
                needed < all,
                "{fix_id} still asks for every collector ({needed}/{all})"
            );
        }
    }

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    const FP: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn apply_of(id: &str) -> Action {
        Action::Apply {
            fix_id: id.into(),
            fingerprint: FP.into(),
        }
    }

    #[test]
    fn apply_accepts_only_known_fix_ids() {
        assert_eq!(
            parse_request(FixTier::Routine, &args(&["apply", "fix.enable_ufw", FP]))
                .unwrap()
                .action,
            apply_of("fix.enable_ufw")
        );
        // The id is the only free-form input, and it is validated against the
        // compiled-in registry: anything else is rejected before any root work.
        for hostile in [
            "fix.enable_ufw; rm -rf /",
            "../../etc/passwd",
            "fix.nonexistent",
        ] {
            assert!(parse_request(FixTier::Routine, &args(&["apply", hostile, FP])).is_err());
            assert!(parse_request(FixTier::Destructive, &args(&["apply", hostile, FP])).is_err());
        }
    }

    #[test]
    fn apply_requires_a_well_formed_fingerprint() {
        // The fingerprint is the second free-form input; it is checked for
        // shape before anything runs, exactly like the id.
        for bad in [
            "",
            "abc",
            &"A".repeat(64),
            &"g".repeat(64),
            "fix.enable_ufw",
            "22/tcp; rm -rf /",
        ] {
            let err = parse_request(FixTier::Routine, &args(&["apply", "fix.enable_ufw", bad]))
                .expect_err("malformed fingerprint accepted");
            assert!(err.contains("fingerprint"), "unhelpful error: {err}");
        }
        // And it is mandatory: the two-argument form that predates the
        // consent check is no longer an apply.
        assert!(parse_request(FixTier::Routine, &args(&["apply", "fix.enable_ufw"])).is_err());
    }

    #[test]
    fn a_helper_refuses_fixes_from_the_other_tier() {
        // polkit authorized *this* class of change. Running the other class
        // under that authorization would make the prompt a lie, so each binary
        // rejects the other tier's ids outright.
        let err = parse_request(FixTier::Routine, &args(&["apply", "fix.autoremove", FP]))
            .expect_err("routine helper must refuse a destructive fix");
        assert!(err.contains("only runs"), "unhelpful error: {err}");

        let err = parse_request(
            FixTier::Destructive,
            &args(&["apply", "fix.enable_ufw", FP]),
        )
        .expect_err("destructive helper must refuse a routine fix");
        assert!(err.contains("only runs"), "unhelpful error: {err}");

        // Each accepts its own.
        assert!(parse_request(
            FixTier::Destructive,
            &args(&["apply", "fix.autoremove", FP])
        )
        .is_ok());
        assert!(parse_request(FixTier::Routine, &args(&["apply", "fix.enable_ufw", FP])).is_ok());
    }

    #[test]
    fn apply_requires_exactly_one_id_and_one_fingerprint() {
        assert!(parse_request(FixTier::Routine, &args(&["apply"])).is_err());
        assert!(parse_request(
            FixTier::Routine,
            &args(&["apply", "fix.enable_ufw", FP, "extra"])
        )
        .is_err());
    }

    #[test]
    fn undo_and_list_take_no_arguments() {
        assert_eq!(
            parse_request(FixTier::Routine, &args(&["undo"]))
                .unwrap()
                .action,
            Action::Undo
        );
        assert_eq!(
            parse_request(FixTier::Routine, &args(&["list-journal"]))
                .unwrap()
                .action,
            Action::ListJournal
        );
        assert!(parse_request(FixTier::Routine, &args(&["undo", "x"])).is_err());
    }

    #[test]
    fn unknown_verbs_are_rejected() {
        assert!(parse_request(FixTier::Routine, &args(&["delete-everything"])).is_err());
        assert!(parse_request(FixTier::Routine, &args(&[])).is_err());
    }

    #[test]
    fn the_language_rides_as_a_validated_trailing_argument() {
        let req = parse_request(FixTier::Routine, &args(&["undo", "--lang=ar"])).unwrap();
        assert_eq!(req.action, Action::Undo);
        assert_eq!(req.lang, Some(Lang::Ar));

        let req = parse_request(
            FixTier::Routine,
            &args(&["apply", "fix.enable_ufw", FP, &lang_arg(Lang::En)]),
        )
        .unwrap();
        assert_eq!(req.lang, Some(Lang::En));

        // Absent: the helper falls back to its environment.
        assert_eq!(
            parse_request(FixTier::Routine, &args(&["undo"]))
                .unwrap()
                .lang,
            None
        );
        // Anything but the two supported tags is refused, not guessed.
        for bad in ["--lang=fr", "--lang=", "--lang=ar; id", "--lang=AR"] {
            assert!(
                parse_request(FixTier::Routine, &args(&["undo", bad])).is_err(),
                "{bad} was accepted"
            );
        }
        // And it must be last: it is not a verb.
        assert!(parse_request(FixTier::Routine, &args(&["--lang=ar", "undo"])).is_err());
    }

    #[test]
    fn the_plan_changed_message_exists_in_both_languages() {
        assert!(plan_changed_message(Lang::En).contains("preview"));
        assert!(plan_changed_message(Lang::Ar)
            .chars()
            .any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)));
    }
}
