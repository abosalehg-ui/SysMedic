//! Fix implementations for SysMedic.
//!
//! M1 ships the contract only (see [`sysmedic_core::fix`]): findings carry a
//! `fix_hint` command the user can run manually. In M3 this crate gains the
//! actual [`Fixer`] implementations, executed through the privileged
//! `sysmedicd` D-Bus helper with polkit authorization, a mandatory preview
//! (what happens, which files change, is it reversible) and an undo journal.

pub use sysmedic_core::fix::{FixPlan, Fixer};

/// Fix ids planned for M3, in priority order.
pub const PLANNED_FIXES: &[&str] = &[
    "fix.apt_clean",
    "fix.journal_vacuum",
    "fix.apt_autoremove_kernels",
    "fix.snap_retain_two",
    "fix.flatpak_remove_unused",
    "fix.disable_service",
    "fix.enable_ufw",
];
