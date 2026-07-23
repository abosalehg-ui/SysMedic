//! sysmedicd — placeholder binary.
//!
//! In M3 this becomes the privileged D-Bus helper (polkit-authorized fix
//! execution with an undo journal); in M5 it gains the checkup scheduler
//! (systemd timers) and desktop notifications. See docs/ROADMAP.md.

fn main() {
    eprintln!(
        "sysmedicd is not implemented yet — it arrives in milestone M3 (see docs/ROADMAP.md)."
    );
    std::process::exit(1);
}
