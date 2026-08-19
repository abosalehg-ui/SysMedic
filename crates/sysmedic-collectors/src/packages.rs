//! Package hygiene, behind a small package-manager abstraction.
//!
//! The collector detects which package manager owns the system (dpkg/apt,
//! dnf or pacman) and runs that manager's own tools. Debian-family systems
//! get the fullest coverage (broken packages, old kernels, cache size,
//! security updates); dnf and pacman systems get correct upgradable/broken
//! counts instead of the misleading "dpkg not found" error they used to see.
//! Anything a manager cannot report cheaply stays `None`/empty and the
//! matching rules simply don't fire.

use sysmedic_core::snapshot::PackageInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

/// The package managers SysMedic knows how to query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manager {
    Apt,
    Dnf,
    Pacman,
}

impl Manager {
    /// The value stored in `PackageInfo::manager` (stable, machine-readable).
    pub fn id(self) -> &'static str {
        match self {
            Manager::Apt => "apt",
            Manager::Dnf => "dnf",
            Manager::Pacman => "pacman",
        }
    }
}

/// Detect the system's package manager. dpkg is probed first: dnf-based
/// distros never ship dpkg, but Debian systems may have a stray pacman/dnf
/// binary from tooling.
fn detect() -> Option<Manager> {
    if util::run("dpkg", &["--version"]).is_some() {
        Some(Manager::Apt)
    } else if util::run("dnf", &["--version"]).is_some() {
        Some(Manager::Dnf)
    } else if util::run("pacman", &["--version"]).is_some() {
        Some(Manager::Pacman)
    } else {
        None
    }
}

pub struct PackageCollector;

impl Collector for PackageCollector {
    fn name(&self) -> &'static str {
        "packages"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let Some(manager) = detect() else {
            snapshot
                .collection_errors
                .push("packages: no supported package manager found (dpkg/dnf/pacman)".into());
            return;
        };
        let info = match manager {
            Manager::Apt => collect_apt(),
            Manager::Dnf => collect_dnf(),
            Manager::Pacman => collect_pacman(),
        };
        snapshot.packages = Some(info);
    }
}

fn collect_apt() -> PackageInfo {
    let mut info = PackageInfo {
        manager: Some(Manager::Apt.id().into()),
        ..Default::default()
    };

    // `dpkg --audit` prints nothing when the database is consistent.
    if let Some(audit) = util::run("dpkg", &["--audit"]) {
        info.broken = parse_dpkg_audit(&audit);
    }

    let running = util::read_trimmed("/proc/sys/kernel/osrelease").unwrap_or_default();
    if let Some(list) = util::run(
        "dpkg-query",
        &["-W", "-f=${Package}\t${Status}\n", "linux-image-*"],
    ) {
        info.old_kernels = parse_old_kernels(&list, &running);
    }

    info.apt_cache_bytes = Some(util::dir_size("/var/cache/apt/archives", 1));

    if let Some(upgradable) = util::run("apt", &["list", "--upgradable"]) {
        let (total, security) = parse_upgradable(&upgradable);
        info.upgradable = Some(total);
        info.security_upgrades = Some(security);
        // Those two counts are only as current as the package index they were
        // computed from, so record its age alongside them.
        info.index_age_days = index_age_days(APT_INDEX_STAMPS);
    }

    // What `autoremove --purge` would actually delete. Simulated (`-s` needs
    // no privileges) so the fix preview can name the packages instead of
    // implying only old kernels are affected.
    if let Some(sim) = util::run_captured("apt-get", &["-s", "autoremove", "--purge"]) {
        info.autoremovable = parse_autoremovable(&sim.stdout);
    }
    info
}

/// Files apt updates when the package index is refreshed, newest wins.
///
/// `update-success-stamp` is written by Debian/Ubuntu's periodic updater and
/// is the most direct signal; `/var/lib/apt/lists` is touched by any
/// `apt update`, including a manual one, so it covers systems where the timer
/// is off. `/var/lib/apt/periodic/update-stamp` records an attempt (successful
/// or not) and is the last resort.
const APT_INDEX_STAMPS: &[&str] = &[
    "/var/lib/apt/periodic/update-success-stamp",
    "/var/lib/apt/lists",
    "/var/lib/apt/periodic/update-stamp",
];

/// dnf's metadata cache; `--cacheonly` reads exactly this.
const DNF_INDEX_STAMPS: &[&str] = &["/var/cache/dnf", "/var/cache/libdnf5"];

/// pacman's synced databases, refreshed by `pacman -Sy`.
const PACMAN_INDEX_STAMPS: &[&str] = &["/var/lib/pacman/sync"];

/// How many days ago the newest of `stamps` was modified, or `None` when none
/// is readable (so the rule stays quiet rather than guessing).
fn index_age_days(stamps: &[&str]) -> Option<u64> {
    let newest = stamps
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok()?.modified().ok())
        .max()?;
    let elapsed = std::time::SystemTime::now().duration_since(newest).ok()?;
    Some(elapsed.as_secs() / 86_400)
}

/// Package names from `apt-get -s autoremove --purge`.
///
/// The simulation prints one `Remv <name> [version]` line per package it
/// would delete (`Purg` when configuration files go too).
pub fn parse_autoremovable(s: &str) -> Vec<String> {
    s.lines()
        .filter_map(|line| {
            let mut tokens = line.split_whitespace();
            match tokens.next()? {
                "Remv" | "Purg" => tokens.next().map(String::from),
                _ => None,
            }
        })
        .collect()
}

fn collect_dnf() -> PackageInfo {
    let mut info = PackageInfo {
        manager: Some(Manager::Dnf.id().into()),
        ..Default::default()
    };

    // `check-update` exits 100 when updates exist, so capture regardless of
    // exit status. `--cacheonly` keeps the checkup off the network.
    if let Some(out) = util::run_captured("dnf", &["-q", "--cacheonly", "check-update"]) {
        info.upgradable = Some(parse_dnf_check_update(&out.stdout));
    }

    // `dnf check` reports rpmdb inconsistencies (non-zero exit when found).
    if let Some(out) = util::run_captured("dnf", &["-q", "check"]) {
        info.broken = parse_dnf_check(&out.stdout);
    }

    // Security advisories among the pending updates (cached metadata only).
    if let Some(out) = util::run_captured(
        "dnf",
        &["-q", "--cacheonly", "updateinfo", "--list", "--security"],
    ) {
        info.security_upgrades = Some(parse_dnf_security(&out.stdout));
    }
    // `--cacheonly` deliberately keeps the checkup off the network, which
    // makes the age of that cache part of the answer.
    info.index_age_days = index_age_days(DNF_INDEX_STAMPS);
    info
}

fn collect_pacman() -> PackageInfo {
    let mut info = PackageInfo {
        manager: Some(Manager::Pacman.id().into()),
        ..Default::default()
    };

    // `pacman -Qu` lists pending upgrades against the last-synced database
    // (exit 1 with no output when there are none).
    if let Some(out) = util::run_captured("pacman", &["-Qu"]) {
        info.upgradable = Some(parse_pacman_upgrades(&out.stdout));
    }

    // `pacman -Dk` checks local database consistency; problem lines mention
    // the affected package.
    if let Some(out) = util::run_captured("pacman", &["-Dk"]) {
        info.broken = parse_pacman_dk(&out.stdout);
    }

    // pacman never prunes its cache by itself: every version of every package
    // ever installed stays in /var/cache/pacman/pkg until cleaned.
    info.pacman_cache_bytes = Some(util::dir_size("/var/cache/pacman/pkg", 1));
    // `pacman -Qu` compares against the last synced database, so the same
    // staleness caveat applies as on apt.
    info.index_age_days = index_age_days(PACMAN_INDEX_STAMPS);
    info
}

/// Package names from `dpkg --audit` output: packages appear as the first
/// token of indented lines under each problem section.
pub fn parse_dpkg_audit(s: &str) -> Vec<String> {
    s.lines()
        .filter(|l| l.starts_with(' ') && !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .map(String::from)
        .collect()
}

/// Installed kernel image packages that are not the running kernel and not
/// meta-packages (`linux-image-generic` etc.).
pub fn parse_old_kernels(dpkg_query: &str, running_release: &str) -> Vec<String> {
    dpkg_query
        .lines()
        .filter_map(|l| {
            let (pkg, status) = l.split_once('\t')?;
            if !status.contains("install ok installed") {
                return None;
            }
            // Versioned kernel packages look like linux-image-6.8.0-45-generic.
            let versioned = pkg
                .strip_prefix("linux-image-")
                .map(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()))
                .unwrap_or(false);
            if !versioned || (!running_release.is_empty() && pkg.contains(running_release)) {
                return None;
            }
            Some(pkg.to_string())
        })
        .collect()
}

/// Count of upgradable packages and how many come from a security pocket,
/// from `apt list --upgradable`.
pub fn parse_upgradable(s: &str) -> (u32, u32) {
    let mut total = 0;
    let mut security = 0;
    for line in s.lines() {
        if !line.contains("[upgradable from") {
            continue;
        }
        total += 1;
        if line.contains("-security") {
            security += 1;
        }
    }
    (total, security)
}

/// Count of pending updates from `dnf -q check-update`: one
/// `name.arch  version  repo` line per package. "Obsoleting Packages"
/// sections and their entries are skipped, as are blank lines.
pub fn parse_dnf_check_update(s: &str) -> u32 {
    let mut count = 0;
    let mut in_obsoleting = false;
    for line in s.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with("Obsoleting Packages") {
            in_obsoleting = true;
            continue;
        }
        if in_obsoleting {
            continue;
        }
        // Update rows have exactly name.arch / version / repo columns and
        // start at column zero (continuation/notice lines are indented).
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() == 3 && !line.starts_with(' ') && cols[0].contains('.') {
            count += 1;
        }
    }
    count
}

/// Problem package names from `dnf -q check` (lines look like
/// `pkg-1.0-1.x86_64 has missing requires of libfoo`).
pub fn parse_dnf_check(s: &str) -> Vec<String> {
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .map(String::from)
        .collect()
}

/// Count of security advisories from
/// `dnf -q updateinfo --list --security`: one `ADVISORY-ID severity/type pkg`
/// line per affected update (header lines like "Last metadata…" are skipped).
pub fn parse_dnf_security(s: &str) -> u32 {
    s.lines()
        .filter(|l| {
            let mut cols = l.split_whitespace();
            // Advisory rows have >= 3 columns and an id like FEDORA-2026-xxxx.
            matches!(cols.next(), Some(first) if first.contains('-') && first.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
                && cols.next().is_some()
                && cols.next().is_some()
        })
        .count() as u32
}

/// Count of pending upgrades from `pacman -Qu`
/// (`name current -> new` per line; `[ignored]` entries are skipped).
pub fn parse_pacman_upgrades(s: &str) -> u32 {
    s.lines()
        .filter(|l| !l.trim().is_empty() && !l.contains("[ignored]"))
        .count() as u32
}

/// Affected package names from `pacman -Dk` problem lines, e.g.
/// `error: dependency 'libfoo' of package bar is missing`. A healthy
/// database prints nothing (or a trailing summary starting with a digit).
pub fn parse_pacman_dk(s: &str) -> Vec<String> {
    s.lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("error:")?;
            let after = rest.split(" of package ").nth(1)?;
            after.split_whitespace().next().map(String::from)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_of_healthy_system_is_empty() {
        assert!(parse_dpkg_audit("").is_empty());
    }

    #[test]
    fn autoremove_simulation_lists_every_package_not_just_kernels() {
        // The point of collecting this: `autoremove --purge` removes far more
        // than the old kernels the fix title used to name.
        let sim = "\
NOTE: This is only a simulation!
Reading package lists...
Building dependency tree...
The following packages will be REMOVED:
  libpython3.11-minimal* linux-image-6.8.0-45-generic* nodejs-doc*
Remv libpython3.11-minimal [3.11.9-1]
Purg linux-image-6.8.0-45-generic [6.8.0-45.45]
Remv nodejs-doc [18.19.0]
";
        assert_eq!(
            parse_autoremovable(sim),
            vec![
                "libpython3.11-minimal",
                "linux-image-6.8.0-45-generic",
                "nodejs-doc"
            ]
        );
    }

    #[test]
    fn autoremove_simulation_with_nothing_to_do_is_empty() {
        let sim = "Reading package lists...\n0 upgraded, 0 newly installed, 0 to remove.\n";
        assert!(parse_autoremovable(sim).is_empty());
    }

    #[test]
    fn audit_extracts_package_names() {
        let fixture = "The following packages are only half configured:\n libfoo1 a broken library\n bar-tools some tools\n";
        assert_eq!(parse_dpkg_audit(fixture), vec!["libfoo1", "bar-tools"]);
    }

    #[test]
    fn old_kernels_exclude_running_and_meta() {
        let fixture = "linux-image-6.8.0-40-generic\tinstall ok installed\nlinux-image-6.8.0-45-generic\tinstall ok installed\nlinux-image-generic\tinstall ok installed\nlinux-image-5.15.0-1-generic\tdeinstall ok config-files\n";
        let old = parse_old_kernels(fixture, "6.8.0-45-generic");
        assert_eq!(old, vec!["linux-image-6.8.0-40-generic"]);
    }

    #[test]
    fn counts_upgradable_and_security() {
        let fixture = "Listing...\nbash/noble-updates 5.2-1 amd64 [upgradable from: 5.1-1]\nopenssl/noble-security 3.0.2 amd64 [upgradable from: 3.0.1]\n";
        assert_eq!(parse_upgradable(fixture), (2, 1));
    }

    #[test]
    fn counts_dnf_updates_and_skips_obsoleting() {
        let fixture = "\
firefox.x86_64                 128.0-1.fc40                 updates\n\
kernel-core.x86_64             6.9.7-200.fc40               updates\n\
\n\
Obsoleting Packages\n\
grub2-tools.x86_64             1:2.06-123.fc40              updates\n";
        assert_eq!(parse_dnf_check_update(fixture), 2);
        assert_eq!(parse_dnf_check_update(""), 0);
    }

    #[test]
    fn dnf_check_lists_problem_packages() {
        let fixture = "bar-2.0-1.x86_64 has missing requires of libfoo.so.1\n";
        assert_eq!(parse_dnf_check(fixture), vec!["bar-2.0-1.x86_64"]);
        assert!(parse_dnf_check("\n").is_empty());
    }

    #[test]
    fn counts_dnf_security_advisories() {
        let fixture = "\
Last metadata expiration check: 0:20:11 ago on Sat 26 Jul 2026.\n\
FEDORA-2026-0a1b2c3d4e Important/Sec. openssl-3.2.1-2.fc40.x86_64\n\
FEDORA-2026-5f6a7b8c9d Moderate/Sec.  curl-8.6.0-3.fc40.x86_64\n";
        assert_eq!(parse_dnf_security(fixture), 2);
        assert_eq!(parse_dnf_security(""), 0);
    }

    #[test]
    fn counts_pacman_upgrades_and_skips_ignored() {
        let fixture = "linux 6.9.6.arch1-1 -> 6.9.7.arch1-1\n\
                       firefox 127.0-1 -> 128.0-1 [ignored]\n";
        assert_eq!(parse_pacman_upgrades(fixture), 1);
        assert_eq!(parse_pacman_upgrades(""), 0);
    }

    #[test]
    fn pacman_dk_extracts_affected_package() {
        let fixture = "error: dependency 'libfoo' of package bar is missing\n";
        assert_eq!(parse_pacman_dk(fixture), vec!["bar"]);
        assert!(parse_pacman_dk("").is_empty());
    }
}
