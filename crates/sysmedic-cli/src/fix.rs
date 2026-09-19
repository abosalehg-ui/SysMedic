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

use crate::text::tools;

/// The journal this process may write (root only, in practice).
fn journal() -> Result<Journal> {
    let path = fixes::journal_path().ok_or_else(|| {
        anyhow::anyhow!("no journal location: neither HOME nor XDG_STATE_HOME is set")
    })?;
    Ok(Journal::load(path)?)
}

/// The journal to *read* for a preview.
///
/// Fixes applied through pkexec land in the system journal, because the helper
/// that applied them was root. Previewing through the writable path sent an
/// unprivileged `sysmedic undo` to the per-user file nothing ever writes, so it
/// answered "Nothing to undo" and then `--yes` went on to undo something.
fn readable_journal() -> Result<Journal> {
    let path = fixes::journal_read_path().ok_or_else(|| {
        anyhow::anyhow!("no journal location: neither HOME nor XDG_STATE_HOME is set")
    })?;
    Ok(Journal::load(path)?)
}

fn collect() -> Snapshot {
    sysmedic_collectors::default_snapshot()
}

/// `sysmedic fix` with no id: list every fix that applies right now.
pub fn list(lang: Lang) -> Result<()> {
    let t = tools(lang);
    let snapshot = collect();
    let plans = fixes::applicable_plans(&snapshot);
    if plans.is_empty() {
        println!(
            "{}",
            t.no_fixes.if_supports_color(Stream::Stdout, |t| t.green())
        );
        return Ok(());
    }
    println!("{}\n", t.applicable_fixes);
    for plan in plans {
        let rev = if plan.reversible {
            t.reversible
                .if_supports_color(Stream::Stdout, |t| t.green())
                .to_string()
        } else {
            t.not_reversible
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
pub fn apply(id: &str, dry_run: bool, yes: bool, lang: Lang) -> Result<()> {
    let t = tools(lang);
    let snapshot = collect();
    let Some(plan) = fixes::plan(id, &snapshot) else {
        bail!("fix '{id}' is unknown or not applicable right now (see `sysmedic fix`)");
    };

    // The consent preview follows the user's locale, like the knowledge base.
    println!("{}", plan.preview_in(lang));

    if dry_run {
        println!(
            "{}",
            t.dry_run_note
                .if_supports_color(Stream::Stdout, |t| t.dimmed())
        );
        return Ok(());
    }
    if !yes {
        println!(
            "{}",
            t.rerun_to_apply
                .if_supports_color(Stream::Stdout, |t| t.cyan())
        );
        return Ok(());
    }

    if fixes::is_root() {
        let mut journal = journal()?;
        let outcome = fixes::apply(&plan, &RealRunner, &mut journal)?;
        println!(
            "{} {} {}.",
            "✓".if_supports_color(Stream::Stdout, |t| t.green()),
            t.applied,
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
        delegate(&helper, &["apply", id], lang)
    }
}

/// `sysmedic undo`: revert the most recent reversible fix.
pub fn undo(yes: bool, lang: Lang) -> Result<()> {
    let t = tools(lang);
    if !yes {
        // Preview from the journal the *helper* writes, not the one this
        // unprivileged process could write.
        match readable_journal() {
            Ok(journal) => match journal.last_undoable() {
                Some((_, entry)) => {
                    let title =
                        fixes::undo_title_in(&journal, lang).unwrap_or_else(|| entry.title.clone());
                    println!(
                        "{} {} ({})",
                        t.would_undo,
                        title.if_supports_color(Stream::Stdout, |t| t.bold()),
                        entry.fix_id
                    );
                    println!(
                        "{}",
                        t.rerun_to_undo
                            .if_supports_color(Stream::Stdout, |t| t.cyan())
                    );
                }
                None => println!(
                    "{}",
                    t.nothing_to_undo
                        .if_supports_color(Stream::Stdout, |t| t.green())
                ),
            },
            Err(e) => println!("{} {e})", t.cannot_read_journal),
        }
        return Ok(());
    }

    if fixes::is_root() {
        let mut journal = journal()?;
        let title = fixes::undo(&RealRunner, &mut journal, lang)?;
        println!(
            "{} {} {}.",
            "✓".if_supports_color(Stream::Stdout, |t| t.green()),
            t.reverted,
            title.if_supports_color(Stream::Stdout, |t| t.bold())
        );
        Ok(())
    } else {
        delegate(&fixes::undo_helper(), &["undo"], lang)
    }
}

/// Replace this process with `pkexec <helper> <args...>` so polkit authorizes
/// the privileged step. Never returns on success.
fn delegate(helper: &str, args: &[&str], lang: Lang) -> Result<()> {
    eprintln!(
        "{}",
        format!(
            "{} (pkexec {helper} {})…",
            tools(lang).requesting_auth,
            args.join(" ")
        )
        .dimmed()
    );
    let err = Command::new("pkexec").arg(helper).args(args).exec();
    // exec only returns on failure.
    bail!("could not launch pkexec: {err}. Is polkit installed and the helper at {helper}?");
}
