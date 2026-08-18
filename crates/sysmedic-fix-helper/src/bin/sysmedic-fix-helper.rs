//! The routine privileged helper: undoable setting changes only.
//!
//! Authorized by the polkit action `io.github.abosalehg_ui.sysmedic.run-fix`,
//! whose prompt says the change can be undone. Ids belonging to the
//! destructive tier are refused here even though this binary runs as root —
//! the authorization the user granted was for this class of change.

use std::process::ExitCode;

fn main() -> ExitCode {
    sysmedic_fix_helper::main_for(sysmedic_fixes::FixTier::Routine)
}
