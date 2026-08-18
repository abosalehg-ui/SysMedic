//! Diagnostic rules for SysMedic.
//!
//! Every rule is a pure function `fn(&Snapshot) -> Vec<Finding>` — no I/O —
//! so each is unit-tested against fixture snapshots. Finding ids are stable
//! and each has a matching entry in the sysmedic-knowledge base.

pub mod rules;

use sysmedic_core::{Diagnostic, Engine, Finding, Snapshot};

/// The standard checkup engine: default collectors + the full rule set.
/// The single composition root — CLI, GUI and the fix helper previously each
/// rebuilt this by hand, which is exactly how their configurations drift.
pub fn default_engine() -> Engine {
    Engine::new()
        .with_collectors(sysmedic_collectors::default_collectors())
        .with_diagnostics(default_diagnostics())
}

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
            "packages-pacman-cache-large",
            rules::packages::pacman_cache_large
        ),
        rule!(
            "security-updates-pending",
            rules::packages::security_updates
        ),
        rule!("security-index-stale", rules::packages::index_stale),
        rule!(
            "packages-upgrades-pending",
            rules::packages::upgrades_pending
        ),
        rule!("logs-journal-large", rules::logs::journal_large),
        rule!("logs-large-files", rules::logs::large_files),
        rule!("storage-snap-old-revisions", rules::snap::old_revisions),
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
    "packages.pacman_cache_large",
    "security.updates_pending",
    "security.index_stale",
    "packages.upgrades_pending",
    "logs.journal_large",
    "logs.large_files",
    "storage.snap_old_revisions",
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

#[cfg(test)]
mod doc_guards {
    /// The rule count is quoted in prose that no compiler checks. It had
    /// drifted to "27" in the README and both site pages, and to "21" in
    /// ISSUES.md and ROADMAP.md, while the code declared 28.
    ///
    /// When this fails, update the number here **and** in:
    ///   README.md · docs/site/index.html · docs/site/ar.html
    ///   docs/ISSUES.md · docs/ROADMAP.md
    #[test]
    fn declared_rule_count_matches_the_documentation() {
        assert_eq!(
            super::FINDING_IDS.len(),
            29,
            "rule count changed — update the docs listed above"
        );
    }

    #[test]
    fn every_rule_emits_a_declared_id_and_ids_are_unique() {
        let mut ids: Vec<&str> = super::FINDING_IDS.to_vec();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate id in FINDING_IDS");
        assert_eq!(
            super::default_diagnostics().len(),
            before,
            "rule count and declared-id count disagree"
        );
    }

    #[test]
    fn finding_id_prefixes_match_their_category() {
        // The prefix is the category the finding is scored under. Two ids used
        // to disagree (`packages.security_updates` scored as Security,
        // `snap.old_revisions` as Storage), which broke grouping by prefix.
        use sysmedic_core::Category;
        let expected = |c: Category| match c {
            Category::Boot => "boot",
            Category::Cpu => "cpu",
            Category::Memory => "memory",
            Category::Storage => "storage",
            Category::Thermal => "thermal",
            Category::Processes => "processes",
            Category::Services => "services",
            Category::Packages => "packages",
            Category::Logs => "logs",
            Category::Network => "network",
            Category::Security => "security",
            Category::Battery => "battery",
        };
        // Fire every rule against a deliberately unhealthy snapshot.
        let s = crate::rules::tests_support::unhealthy_snapshot();
        let mut checked = 0;
        for rule in super::default_diagnostics() {
            for finding in rule.evaluate(&s) {
                let prefix = finding.id.split('.').next().unwrap_or("");
                // `smart.*` and `flatpak.*` name the data source rather than
                // the category, which is a deliberate and consistent choice.
                if matches!(prefix, "smart" | "flatpak") {
                    continue;
                }
                assert_eq!(
                    prefix,
                    expected(finding.category),
                    "id {} is scored under {:?}",
                    finding.id,
                    finding.category
                );
                checked += 1;
            }
        }
        assert!(checked > 15, "fixture fired too few rules ({checked})");
    }
}
