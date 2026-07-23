//! SysMedic GUI — placeholder binary.
//!
//! The GTK4/libadwaita application (dashboard, health score, findings with
//! explanations, dark/light) is milestone M2 so this crate carries no GTK
//! dependency yet. Until then, use the CLI: `sysmedic checkup`.

fn main() {
    eprintln!("The SysMedic GUI arrives in milestone M2 (see docs/ROADMAP.md).");
    eprintln!("In the meantime run: sysmedic checkup");
    std::process::exit(1);
}
