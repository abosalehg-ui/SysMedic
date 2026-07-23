//! Diagnostic rules for SysMedic.
//!
//! Every rule is a pure function `fn(&Snapshot) -> Vec<Finding>` — no I/O —
//! so each is unit-tested against fixture snapshots. Finding ids are stable
//! and each has a matching entry in the sysmedic-knowledge base.

pub mod rules;

use sysmedic_core::{Diagnostic, Finding, Snapshot};

struct Rule {
    name: &'static str,
    check: fn(&Snapshot) -> Vec<Finding>,
}

impl Diagnostic for Rule {
    fn name(&self) -> &'static str {
        self.name
    }
    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding> {
        (self.check)(snapshot)
    }
}

macro_rules! rule {
    ($name:literal, $f:path) => {
        Box::new(Rule {
            name: $name,
            check: $f,
        }) as Box<dyn Diagnostic>
    };
}

/// The full v1 rule set.
pub fn default_diagnostics() -> Vec<Box<dyn Diagnostic>> {
    vec![
        rule!("disk-nearly-full", rules::storage::disk_nearly_full),
        rule!("memory-low-available", rules::memory::low_available),
        rule!("memory-swap-pressure", rules::memory::swap_pressure),
        rule!("cpu-high-load", rules::cpu::high_load),
        rule!("thermal-overheating", rules::thermal::overheating),
        rule!("processes-zombies", rules::processes::zombies),
        rule!("services-failed", rules::services::failed_units),
        rule!("boot-slow", rules::boot::slow_boot),
        rule!("packages-broken", rules::packages::broken),
        rule!("packages-old-kernels", rules::packages::old_kernels),
        rule!("packages-apt-cache-large", rules::packages::apt_cache_large),
        rule!(
            "packages-security-updates",
            rules::packages::security_updates
        ),
        rule!(
            "packages-upgrades-pending",
            rules::packages::upgrades_pending
        ),
        rule!("logs-journal-large", rules::logs::journal_large),
        rule!("logs-large-files", rules::logs::large_files),
        rule!("snap-old-revisions", rules::snap::old_revisions),
        rule!("flatpak-unused-runtimes", rules::flatpak::unused_runtimes),
        rule!("battery-degraded", rules::battery::degraded),
        rule!("network-no-default-route", rules::network::no_default_route),
        rule!("network-no-dns", rules::network::no_dns),
        rule!("security-ssh-root-login", rules::security::ssh_root_login),
        rule!(
            "security-firewall-inactive",
            rules::security::firewall_inactive
        ),
        rule!(
            "security-ssh-password-auth",
            rules::security::ssh_password_auth
        ),
        rule!("security-exposed-ports", rules::security::exposed_ports),
        rule!("smart-failing", rules::smart::failing),
        rule!(
            "smart-reallocated-sectors",
            rules::smart::reallocated_sectors
        ),
        rule!("smart-ssd-wear", rules::smart::ssd_wear),
    ]
}

/// Every finding id a rule can emit. The knowledge base is tested against
/// this list so no finding ever lacks an explanation.
pub const FINDING_IDS: &[&str] = &[
    "storage.disk_nearly_full",
    "memory.low_available",
    "memory.swap_pressure",
    "cpu.high_load",
    "thermal.overheating",
    "processes.zombies",
    "services.failed",
    "boot.slow",
    "packages.broken",
    "packages.old_kernels",
    "packages.apt_cache_large",
    "packages.security_updates",
    "packages.upgrades_pending",
    "logs.journal_large",
    "logs.large_files",
    "snap.old_revisions",
    "flatpak.unused_runtimes",
    "battery.degraded",
    "network.no_default_route",
    "network.no_dns",
    "security.ssh_root_login",
    "security.firewall_inactive",
    "security.ssh_password_auth",
    "security.exposed_ports",
    "smart.failing",
    "smart.reallocated_sectors",
    "smart.ssd_wear",
];
