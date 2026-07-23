//! `sysmedic fix` and `sysmedic undo`.
//!
//! The CLI never runs a privileged command itself unless it is already root.
//! When invoked unprivileged it delegates to the pkexec-launched
//! `sysmedic-fix-helper`, so authorization always goes through polkit.

use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{bail, Result};
use owo_colors::OwoColorize;
use sysmedic_core::Snapshot;
use sysmedic_fixes::{self as fixes, Journal, RealRunner};

const DEFAULT_HELPER: &str = "/usr/libexec/sysmedic-fix-helper";

fn helper_path() -> String {
    std::env::var("SYSMEDIC_HELPER").unwrap_or_else(|_| DEFAULT_HELPER.to_string())
}

fn collect() -> Snapshot {
    sysmedic_core::Engine::new()
        .with_collectors(sysmedic_collectors::default_collectors())
        .run()
        .snapshot
}

/// `sysmedic fix` with no id: list every fix that applies right now.
pub fn list() -> Result<()> {
    let snapshot = collect();
    let plans = fixes::applicable_plans(&snapshot);
    if plans.is_empty() {
        println!("{}", "No fixes needed — nothing to prescribe.".green());
        return Ok(());
    }
    println!("Applicable fixes (run `sysmedic fix <id> --dry-run` to preview):\n");
    for plan in plans {
        let rev = if plan.reversible {
            "reversible".green().to_string()
        } else {
            "not reversible".yellow().to_string()
        };
        println!("  {}  [{}]  {}", plan.id.bold(), rev, plan.title);
    }
    Ok(())
}

/// `sysmedic fix <id>`: preview, then (with `--yes`) apply.
pub fn apply(id: &str, dry_run: bool, yes: bool) -> Result<()> {
    let snapshot = collect();
    let Some(plan) = fixes::plan(id, &snapshot) else {
        bail!("fix '{id}' is unknown or not applicable right now (see `sysmedic fix`)");
    };

    println!("{}", plan.preview());

    if dry_run {
        println!("{}", "(dry run — nothing was changed)".dimmed());
        return Ok(());
    }
    if !yes {
        println!("{}", "Re-run with --yes to apply this fix.".cyan());
        return Ok(());
    }

    if fixes::is_root() {
        let mut journal = Journal::load(fixes::journal_path()).map_err(anyhow::Error::msg)?;
        let outcome = fixes::apply(&plan, &RealRunner, &mut journal).map_err(anyhow::Error::msg)?;
        println!("{} applied {}.", "✓".green(), outcome.fix_id.bold());
        for line in outcome.outputs.iter().filter(|l| !l.is_empty()) {
            println!("  {}", line.dimmed());
        }
        Ok(())
    } else {
        delegate(&["apply", id])
    }
}

/// `sysmedic undo`: revert the most recent reversible fix.
pub fn undo(yes: bool) -> Result<()> {
    if !yes {
        // Preview what would be undone from the journal we can read.
        match Journal::load(fixes::journal_path()) {
            Ok(journal) => match journal.last_undoable() {
                Some((_, entry)) => {
                    println!("Would undo: {} ({})", entry.title.bold(), entry.fix_id);
                    println!("{}", "Re-run with --yes to undo.".cyan());
                }
                None => println!("{}", "Nothing to undo.".green()),
            },
            Err(e) => println!("(cannot read journal: {e})"),
        }
        return Ok(());
    }

    if fixes::is_root() {
        let mut journal = Journal::load(fixes::journal_path()).map_err(anyhow::Error::msg)?;
        let title = fixes::undo(&RealRunner, &mut journal).map_err(anyhow::Error::msg)?;
        println!("{} reverted {}.", "✓".green(), title.bold());
        Ok(())
    } else {
        delegate(&["undo"])
    }
}

/// Replace this process with `pkexec <helper> <args...>` so polkit authorizes
/// the privileged step. Never returns on success.
fn delegate(args: &[&str]) -> Result<()> {
    let helper = helper_path();
    eprintln!(
        "{}",
        format!(
            "Requesting authorization (pkexec {helper} {})…",
            args.join(" ")
        )
        .dimmed()
    );
    let err = Command::new("pkexec").arg(&helper).args(args).exec();
    // exec only returns on failure.
    bail!("could not launch pkexec: {err}. Is polkit installed and the helper at {helper}?");
}
