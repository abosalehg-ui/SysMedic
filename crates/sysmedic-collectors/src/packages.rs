use sysmedic_core::snapshot::PackageInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct PackageCollector;

impl Collector for PackageCollector {
    fn name(&self) -> &'static str {
        "packages"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        if util::run("dpkg", &["--version"]).is_none() {
            snapshot
                .collection_errors
                .push("packages: dpkg not found (non-Debian system?)".into());
            return;
        }
        let mut info = PackageInfo::default();

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

        let cache = util::dir_size("/var/cache/apt/archives", 1);
        info.apt_cache_bytes = Some(cache);

        if let Some(upgradable) = util::run("apt", &["list", "--upgradable"]) {
            let (total, security) = parse_upgradable(&upgradable);
            info.upgradable = Some(total);
            info.security_upgrades = Some(security);
        }

        snapshot.packages = Some(info);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_of_healthy_system_is_empty() {
        assert!(parse_dpkg_audit("").is_empty());
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
}
