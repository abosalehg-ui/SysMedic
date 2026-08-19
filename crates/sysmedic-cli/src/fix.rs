//! `sysmedic fix` and `sysmedic undo`.
//!
//! The CLI never runs a privileged command itself unless it is already root.
//! When invoked unprivileged it delegates to the pkexec-launched
//! `sysmedic-fix-helper`, so authorization always goes through polkit.

use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{bail, Result};
use owo_colors::{OwoColorize, Stream};
use sysmedic_core::{Lang, Snapshot};
use sysmedic_fixes::{self as fixes, Journal, RealRunner};

/// Resolve the journal path, or explain why there isn't one.
fn journal() -> Result<Journal> {
    let path = fixes::journal_path().ok_or_else(|| {
        anyhow::anyhow!("no journal location: neither HOME nor XDG_STATE_HOME is set")
    })?;
    Ok(Journal::load(path)?)
}

fn user_lang() -> Lang {
    Lang::from_env()
}

fn collect() -> Snapshot {
    sysmedic_collectors::default_snapshot()
}

/// `sysmedic fix` with no id: list every fix that applies right now.
pub fn list() -> Result<()> {
    let snapshot = collect();
    let plans = fixes::applicable_plans(&snapshot);
    if plans.is_empty() {
        println!(
            "{}",
            "No fixes needed — nothing to prescribe."
                .if_supports_color(Stream::Stdout, |t| t.green())
        );
        return Ok(());
    }
    let lang = user_lang();
    println!("Applicable fixes (run `sysmedic fix <id> --dry-run` to preview):\n");
    for plan in plans {
        let rev = if plan.reversible {
            "reversible"
                .if_supports_color(Stream::Stdout, |t| t.green())
                .to_string()
        } else {
            "not reversible"
                .if_supports_color(Stream::Stdout, |t| t.yellow())
                .to_string()
        };
        println!(
            "  {}  [{}]  {}",
            plan.id.if_supports_color(Stream::Stdout, |t| t.bold()),
            rev,
            plan.title_in(lang)
        );
    }
    Ok(())
}

/// `sysmedic fix <id>`: preview, then (with `--yes`) apply.
pub fn apply(id: &str, dry_run: bool, yes: bool) -> Result<()> {
    let snapshot = collect();
    let Some(plan) = fixes::plan(id, &snapshot) else {
        bail!("fix '{id}' is unknown or not applicable right now (see `sysmedic fix`)");
    };

    // The consent preview follows the user's locale, like the knowledge base.
    println!("{}", plan.preview_in(user_lang()));

    if dry_run {
        println!(
            "{}",
            "(dry run — nothing was changed)".if_supports_color(Stream::Stdout, |t| t.dimmed())
        );
        return Ok(());
    }
    if !yes {
        println!(
            "{}",
            "Re-run with --yes to apply this fix.".if_supports_color(Stream::Stdout, |t| t.cyan())
        );
        return Ok(());
    }

    if fixes::is_root() {
        let mut journal = journal()?;
        let outcome = fixes::apply(&plan, &RealRunner, &mut journal)?;
        println!(
            "{} applied {}.",
            "✓".if_supports_color(Stream::Stdout, |t| t.green()),
            outcome
                .fix_id
                .if_supports_color(Stream::Stdout, |t| t.bold())
        );
        for line in outcome.outputs.iter().filter(|l| !l.is_empty()) {
            println!(
                "  {}",
                line.if_supports_color(Stream::Stdout, |t| t.dimmed())
            );
        }
        Ok(())
    } else {
        // Which helper — and therefore which polkit prompt — depends on
        // whether this fix can be undone.
        let helper =
            fixes::helper_for_fix(id).ok_or_else(|| anyhow::anyhow!("fix '{id}' is unknown"))?;
        delegate(&helper, &["apply", id])
    }
}

/// `sysmedic undo`: revert the most recent reversible fix.
pub fn undo(yes: bool) -> Result<()> {
    if !yes {
        // Preview what would be undone from the journal we can read.
        match journal() {
            Ok(journal) => match journal.last_undoable() {
                Some((_, entry)) => {
                    let title = fixes::undo_title_in(&journal, user_lang())
                        .unwrap_or_else(|| entry.title.clone());
                    println!(
                        "Would undo: {} ({})",
                        title.if_supports_color(Stream::Stdout, |t| t.bold()),
                        entry.fix_id
                    );
                    println!(
                        "{}",
                        "Re-run with --yes to undo."
                            .if_supports_color(Stream::Stdout, |t| t.cyan())
                    );
                }
                None => println!(
                    "{}",
                    "Nothing to undo.".if_supports_color(Stream::Stdout, |t| t.green())
                ),
            },
            Err(e) => println!("(cannot read journal: {e})"),
        }
        return Ok(());
    }

    if fixes::is_root() {
        let mut journal = journal()?;
        let title = fixes::undo(&RealRunner, &mut journal, user_lang())?;
        println!(
            "{} reverted {}.",
            "✓".if_supports_color(Stream::Stdout, |t| t.green()),
            title.if_supports_color(Stream::Stdout, |t| t.bold())
        );
        Ok(())
    } else {
        delegate(&fixes::undo_helper(), &["undo"])
    }
}

/// Replace this process with `pkexec <helper> <args...>` so polkit authorizes
/// the privileged step. Never returns on success.
fn delegate(helper: &str, args: &[&str]) -> Result<()> {
    eprintln!(
        "{}",
        format!(
            "Requesting authorization (pkexec {helper} {})…",
            args.join(" ")
        )
        .dimmed()
    );
    let err = Command::new("pkexec").arg(helper).args(args).exec();
    // exec only returns on failure.
    bail!("could not launch pkexec: {err}. Is polkit installed and the helper at {helper}?");
}
