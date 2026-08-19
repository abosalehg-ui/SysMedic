//! The destructive privileged helper: changes `undo` cannot bring back —
//! deleted caches, trimmed logs, purged packages and their configuration.
//!
//! Authorized by its own polkit action
//! `io.github.abosalehg_ui.sysmedic.run-fix-destructive`, so the prompt names
//! what is really at stake and an administrator can allow the routine class
//! without this one.

use std::process::ExitCode;

fn main() -> ExitCode {
    sysmedic_fix_helper::main_for(sysmedic_fixes::FixTier::Destructive)
}
