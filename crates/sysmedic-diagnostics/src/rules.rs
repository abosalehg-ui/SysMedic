//! The individual diagnostic rules, grouped by health category.

fn gb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

pub mod storage {
    use super::gb;
    use sysmedic_core::thresholds::disk;
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn disk_nearly_full(s: &Snapshot) -> Vec<Finding> {
        let Some(disks) = &s.disks else { return vec![] };
        disks
            .iter()
            .filter_map(|d| {
                let used = d.used_percent();
                let severity = if used >= disk::CRITICAL_PCT {
                    Severity::Critical
                } else if used >= disk::FINDING_PCT {
                    Severity::Medium
                } else {
                    return None;
                };
                Some(
                    Finding::new(
                        "storage.disk_nearly_full",
                        Category::Storage,
                        severity,
                        format!("Filesystem {} is {:.0}% full", d.mount_point, used),
                        format!(
                            "Only {:.1} GiB free of {:.1} GiB on {}.",
                            gb(d.available_bytes),
                            gb(d.total_bytes),
                            d.mount_point
                        ),
                    )
                    // Distro-neutral: this rule fires on any system, so avoid
                    // an apt-only hint. Clearing the package cache is offered
                    // separately by a package-manager-specific fix.
                    .with_fix_hint("sudo journalctl --vacuum-size=200M"),
                )
            })
            .collect()
    }
}

pub mod memory {
    use sysmedic_core::thresholds::memory as mem_th;
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn low_available(s: &Snapshot) -> Vec<Finding> {
        let Some(mem) = &s.memory else { return vec![] };
        let avail = mem.available_percent();
        let severity = if avail < mem_th::CRITICAL_PCT {
            Severity::Critical
        } else if avail < mem_th::LOW_PCT {
            Severity::High
        } else {
            return vec![];
        };
        vec![Finding::new(
            "memory.low_available",
            Category::Memory,
            severity,
            format!("Only {avail:.0}% of RAM is available"),
            format!(
                "{} MiB available of {} MiB total.",
                mem.available_kb / 1024,
                mem.total_kb / 1024
            ),
        )]
    }

    pub fn swap_pressure(s: &Snapshot) -> Vec<Finding> {
        let Some(mem) = &s.memory else { return vec![] };
        let Some(swap_used) = mem.swap_used_percent() else {
            return vec![];
        };
        if swap_used > 50.0 && mem.available_percent() < 20.0 {
            vec![Finding::new(
                "memory.swap_pressure",
                Category::Memory,
                Severity::Medium,
                "System is under memory pressure and swapping heavily",
                format!(
                    "{swap_used:.0}% of swap in use while available RAM is low — the system will feel sluggish."
                ),
            )]
        } else {
            vec![]
        }
    }
}

pub mod cpu {
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn high_load(s: &Snapshot) -> Vec<Finding> {
        let Some(cpu) = &s.cpu else { return vec![] };
        let cores = cpu.logical_cores as f64;
        let severity = if cpu.load_15 > cores * 2.0 {
            Severity::High
        } else if cpu.load_15 > cores {
            Severity::Medium
        } else {
            return vec![];
        };
        vec![Finding::new(
            "cpu.high_load",
            Category::Cpu,
            severity,
            "Sustained high CPU load",
            format!(
                "15-minute load average is {:.2} on {} cores.",
                cpu.load_15, cpu.logical_cores
            ),
        )]
    }
}

pub mod thermal {
    use sysmedic_core::thresholds::thermal as thermal_th;
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn overheating(s: &Snapshot) -> Vec<Finding> {
        let Some(thermal) = &s.thermal else {
            return vec![];
        };
        let Some(hottest) = thermal.hottest() else {
            return vec![];
        };
        let severity = if hottest.temp_c >= thermal_th::CRITICAL_C {
            Severity::Critical
        } else if hottest.temp_c >= thermal_th::HIGH_C {
            Severity::High
        } else {
            return vec![];
        };
        vec![Finding::new(
            "thermal.overheating",
            Category::Thermal,
            severity,
            format!("Sensor {} reads {:.0}°C", hottest.name, hottest.temp_c),
            "The system is running hot; sustained high temperatures throttle performance and shorten hardware life.",
        )]
    }
}

pub mod processes {
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn zombies(s: &Snapshot) -> Vec<Finding> {
        let Some(procs) = &s.processes else {
            return vec![];
        };
        if procs.zombies.is_empty() {
            return vec![];
        }
        let severity = if procs.zombies.len() >= 10 {
            Severity::Medium
        } else {
            Severity::Low
        };
        vec![Finding::new(
            "processes.zombies",
            Category::Processes,
            severity,
            format!("{} zombie process(es) found", procs.zombies.len()),
            "Zombie processes are dead but their parent never collected their exit status.",
        )
        .with_evidence(procs.zombies.clone())]
    }
}

pub mod services {
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn failed_units(s: &Snapshot) -> Vec<Finding> {
        let Some(services) = &s.services else {
            return vec![];
        };
        if services.failed.is_empty() {
            return vec![];
        }
        vec![Finding::new(
            "services.failed",
            Category::Services,
            Severity::High,
            format!("{} systemd unit(s) in failed state", services.failed.len()),
            "Failed units mean something the system was asked to run is broken.",
        )
        .with_evidence(services.failed.clone())
        .with_fix_hint("systemctl status <unit> && journalctl -u <unit> -b")]
    }
}

pub mod boot {
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn slow_boot(s: &Snapshot) -> Vec<Finding> {
        let Some(boot) = &s.boot else { return vec![] };
        let severity = if boot.total_seconds > 120.0 {
            Severity::High
        } else if boot.total_seconds > 60.0 {
            Severity::Medium
        } else {
            return vec![];
        };
        let evidence = boot
            .slowest_units
            .iter()
            .map(|u| format!("{:.1}s {}", u.seconds, u.unit))
            .collect();
        vec![Finding::new(
            "boot.slow",
            Category::Boot,
            severity,
            format!("Boot takes {:.0} seconds", boot.total_seconds),
            "Startup is slower than it should be; the slowest units are listed in the evidence.",
        )
        .with_evidence(evidence)
        .with_fix_hint("systemd-analyze blame")]
    }
}

pub mod packages {
    use super::gb;
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn broken(s: &Snapshot) -> Vec<Finding> {
        let Some(pkgs) = &s.packages else {
            return vec![];
        };
        if pkgs.broken.is_empty() {
            return vec![];
        }
        vec![Finding::new(
            "packages.broken",
            Category::Packages,
            Severity::High,
            format!("{} broken package(s)", pkgs.broken.len()),
            "The dpkg database reports packages in an inconsistent state.",
        )
        .with_evidence(pkgs.broken.clone())
        .with_fix_hint("sudo apt --fix-broken install")]
    }

    pub fn old_kernels(s: &Snapshot) -> Vec<Finding> {
        let Some(pkgs) = &s.packages else {
            return vec![];
        };
        if pkgs.old_kernels.len() <= 2 {
            return vec![];
        }
        vec![Finding::new(
            "packages.old_kernels",
            Category::Packages,
            Severity::Low,
            format!("{} old kernel(s) installed", pkgs.old_kernels.len()),
            "Old kernel images take space in /boot; keeping one fallback is enough.",
        )
        .with_evidence(pkgs.old_kernels.clone())
        .with_fix_hint("sudo apt autoremove --purge")]
    }

    pub fn apt_cache_large(s: &Snapshot) -> Vec<Finding> {
        let Some(pkgs) = &s.packages else {
            return vec![];
        };
        let Some(bytes) = pkgs.apt_cache_bytes else {
            return vec![];
        };
        if bytes < 1024 * 1024 * 1024 {
            return vec![];
        }
        vec![Finding::new(
            "packages.apt_cache_large",
            Category::Packages,
            Severity::Low,
            format!("APT package cache holds {:.1} GiB", gb(bytes)),
            "Downloaded .deb files in /var/cache/apt/archives are safe to delete.",
        )
        .with_fix_hint("sudo apt clean")]
    }

    pub fn security_updates(s: &Snapshot) -> Vec<Finding> {
        let Some(pkgs) = &s.packages else {
            return vec![];
        };
        match pkgs.security_upgrades {
            Some(n) if n > 0 => vec![Finding::new(
                "packages.security_updates",
                Category::Security,
                Severity::High,
                format!("{n} security update(s) pending"),
                "Packages with known security fixes are waiting to be installed.",
            )
            .with_fix_hint("sudo apt update && sudo apt upgrade")],
            _ => vec![],
        }
    }

    pub fn upgrades_pending(s: &Snapshot) -> Vec<Finding> {
        let Some(pkgs) = &s.packages else {
            return vec![];
        };
        match pkgs.upgradable {
            Some(n) if n > 20 => vec![Finding::new(
                "packages.upgrades_pending",
                Category::Packages,
                Severity::Low,
                format!("{n} package update(s) pending"),
                "A large backlog of updates accumulates bugs already fixed upstream.",
            )
            .with_fix_hint("sudo apt update && sudo apt upgrade")],
            _ => vec![],
        }
    }
}

pub mod logs {
    use super::gb;
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn journal_large(s: &Snapshot) -> Vec<Finding> {
        let Some(logs) = &s.logs else { return vec![] };
        let Some(bytes) = logs.journal_bytes else {
            return vec![];
        };
        let severity = if bytes >= 4 * 1024 * 1024 * 1024 {
            Severity::High
        } else if bytes >= 1024 * 1024 * 1024 {
            Severity::Medium
        } else {
            return vec![];
        };
        vec![Finding::new(
            "logs.journal_large",
            Category::Logs,
            severity,
            format!("systemd journal uses {:.1} GiB", gb(bytes)),
            "The journal grows unbounded unless a size limit is set.",
        )
        .with_fix_hint("sudo journalctl --vacuum-size=200M")]
    }

    pub fn large_files(s: &Snapshot) -> Vec<Finding> {
        let Some(logs) = &s.logs else { return vec![] };
        if logs.large_files.is_empty() {
            return vec![];
        }
        let biggest = logs.large_files.iter().map(|f| f.bytes).max().unwrap_or(0);
        let severity = if biggest >= 1024 * 1024 * 1024 {
            Severity::Medium
        } else {
            Severity::Low
        };
        let evidence = logs
            .large_files
            .iter()
            .map(|f| format!("{:.2} GiB {}", gb(f.bytes), f.path))
            .collect();
        vec![Finding::new(
            "logs.large_files",
            Category::Logs,
            severity,
            format!(
                "{} oversized log file(s) in /var/log",
                logs.large_files.len()
            ),
            "A log growing this large usually means a service is erroring in a loop.",
        )
        .with_evidence(evidence)]
    }
}

pub mod snap {
    use super::gb;
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn old_revisions(s: &Snapshot) -> Vec<Finding> {
        let Some(snap) = &s.snap else { return vec![] };
        if snap.disabled_revisions == 0 {
            return vec![];
        }
        let size_note = snap
            .snaps_dir_bytes
            .map(|b| format!(" Snap store on disk: {:.1} GiB.", gb(b)))
            .unwrap_or_default();
        vec![Finding::new(
            "snap.old_revisions",
            Category::Storage,
            Severity::Low,
            format!(
                "{} disabled snap revision(s) kept on disk",
                snap.disabled_revisions
            ),
            format!("Snapd keeps old revisions of every snap after updates.{size_note}"),
        )
        .with_fix_hint("sudo snap set system refresh.retain=2")]
    }
}

pub mod flatpak {
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn unused_runtimes(s: &Snapshot) -> Vec<Finding> {
        let Some(flatpak) = &s.flatpak else {
            return vec![];
        };
        if flatpak.unused_refs.is_empty() {
            return vec![];
        }
        vec![Finding::new(
            "flatpak.unused_runtimes",
            Category::Storage,
            Severity::Low,
            format!(
                "{} unused Flatpak runtime(s) installed",
                flatpak.unused_refs.len()
            ),
            "Flatpak runtimes left behind by removed apps still occupy disk space.",
        )
        .with_evidence(flatpak.unused_refs.clone())
        .with_fix_hint("flatpak uninstall --unused")]
    }
}

pub mod battery {
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn degraded(s: &Snapshot) -> Vec<Finding> {
        let Some(battery) = &s.battery else {
            return vec![];
        };
        let Some(health) = battery.health_percent else {
            return vec![];
        };
        let severity = if health < 40.0 {
            Severity::High
        } else if health < 60.0 {
            Severity::Medium
        } else {
            return vec![];
        };
        vec![Finding::new(
            "battery.degraded",
            Category::Battery,
            severity,
            format!("Battery holds only {health:.0}% of its design capacity"),
            "The battery has aged; runtime on a full charge is significantly reduced.",
        )]
    }
}

pub mod network {
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn no_default_route(s: &Snapshot) -> Vec<Finding> {
        let Some(net) = &s.network else { return vec![] };
        if net.has_default_route {
            return vec![];
        }
        vec![Finding::new(
            "network.no_default_route",
            Category::Network,
            Severity::High,
            "No default network route",
            "Without a default route the system cannot reach the internet.",
        )]
    }

    pub fn no_dns(s: &Snapshot) -> Vec<Finding> {
        let Some(net) = &s.network else { return vec![] };
        if !net.dns_servers.is_empty() {
            return vec![];
        }
        vec![Finding::new(
            "network.no_dns",
            Category::Network,
            Severity::Medium,
            "No DNS servers configured",
            "Name resolution will fail: /etc/resolv.conf lists no nameservers.",
        )]
    }
}

pub mod security {
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn ssh_root_login(s: &Snapshot) -> Vec<Finding> {
        let Some(sec) = &s.security else {
            return vec![];
        };
        if sec.ssh_permit_root_login != Some(true) {
            return vec![];
        }
        vec![Finding::new(
            "security.ssh_root_login",
            Category::Security,
            Severity::High,
            "SSH allows direct root login",
            "sshd_config sets PermitRootLogin yes, exposing the root account to password guessing.",
        )
        .with_fix_hint("set 'PermitRootLogin prohibit-password' in /etc/ssh/sshd_config")]
    }

    pub fn firewall_inactive(s: &Snapshot) -> Vec<Finding> {
        let Some(sec) = &s.security else {
            return vec![];
        };
        if sec.firewall_active != Some(false) {
            return vec![];
        }
        vec![Finding::new(
            "security.firewall_inactive",
            Category::Security,
            Severity::Medium,
            "Firewall is installed but inactive",
            "ufw is present but not enabled; all listening services are exposed to the local network.",
        )
        .with_fix_hint("sudo ufw enable")]
    }

    pub fn ssh_password_auth(s: &Snapshot) -> Vec<Finding> {
        let Some(sec) = &s.security else {
            return vec![];
        };
        if sec.ssh_password_auth != Some(true) {
            return vec![];
        }
        vec![Finding::new(
            "security.ssh_password_auth",
            Category::Security,
            Severity::Medium,
            "SSH accepts password logins",
            "sshd allows password authentication, which is vulnerable to brute-force guessing.",
        )
        .with_fix_hint("use key-based auth and set 'PasswordAuthentication no'")]
    }

    pub fn exposed_ports(s: &Snapshot) -> Vec<Finding> {
        let Some(ports) = &s.ports else { return vec![] };
        let exposed: Vec<String> = ports
            .iter()
            .filter(|p| p.exposed)
            .map(|p| format!("{}/{} on {}", p.port, p.proto, p.address))
            .collect();
        if exposed.is_empty() {
            return vec![];
        }
        vec![Finding::new(
            "security.exposed_ports",
            Category::Security,
            Severity::Low,
            format!("{} service(s) listening on the network", exposed.len()),
            "These ports accept connections from other machines. Make sure each is intended, and let the firewall cover the rest.",
        )
        .with_evidence(exposed)
        .with_fix_hint("review with `ss -tulnp`; enable ufw to gate access")]
    }
}

pub mod smart {
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    pub fn failing(s: &Snapshot) -> Vec<Finding> {
        let Some(devices) = &s.smart else {
            return vec![];
        };
        devices
            .iter()
            .filter(|d| d.health_passed == Some(false))
            .map(|d| {
                Finding::new(
                    "smart.failing",
                    Category::Storage,
                    Severity::Critical,
                    format!("Drive {} reports SMART failure", d.device),
                    format!(
                        "{} ({}) failed its SMART self-assessment — the disk is predicting imminent failure.",
                        d.device, d.model
                    ),
                )
                .with_fix_hint("back up immediately and plan to replace the drive")
            })
            .collect()
    }

    pub fn reallocated_sectors(s: &Snapshot) -> Vec<Finding> {
        let Some(devices) = &s.smart else {
            return vec![];
        };
        devices
            .iter()
            .filter_map(|d| {
                let count = d.reallocated_sectors?;
                if count == 0 {
                    return None;
                }
                let severity = if count >= 50 {
                    Severity::High
                } else {
                    Severity::Medium
                };
                Some(
                    Finding::new(
                        "smart.reallocated_sectors",
                        Category::Storage,
                        severity,
                        format!("{} has {} reallocated sector(s)", d.device, count),
                        "The drive has remapped bad sectors. A rising count signals physical degradation.",
                    )
                    .with_fix_hint("back up important data and monitor the count over time"),
                )
            })
            .collect()
    }

    pub fn ssd_wear(s: &Snapshot) -> Vec<Finding> {
        let Some(devices) = &s.smart else {
            return vec![];
        };
        devices
            .iter()
            .filter_map(|d| {
                let wear = d.wear_percent?;
                if wear < 80 {
                    return None;
                }
                let severity = if wear >= 100 {
                    Severity::High
                } else {
                    Severity::Medium
                };
                Some(
                    Finding::new(
                        "smart.ssd_wear",
                        Category::Storage,
                        severity,
                        format!("SSD {} is {}% through its rated write life", d.device, wear),
                        "NVMe wear indicator is high; the drive is nearing the endurance it was rated for.",
                    )
                    .with_fix_hint("ensure backups; plan replacement as it approaches 100%"),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use sysmedic_core::snapshot::*;
    use sysmedic_core::{Severity, Snapshot};

    fn snapshot() -> Snapshot {
        Snapshot::default()
    }

    #[test]
    fn empty_snapshot_yields_no_findings() {
        let s = snapshot();
        for rule in crate::default_diagnostics() {
            assert!(
                rule.evaluate(&s).is_empty(),
                "rule {} fired on an empty snapshot",
                rule.name()
            );
        }
    }

    #[test]
    fn full_disk_is_critical() {
        let mut s = snapshot();
        s.disks = Some(vec![DiskInfo {
            mount_point: "/".into(),
            fs_type: "ext4".into(),
            total_bytes: 100_000_000_000,
            available_bytes: 3_000_000_000,
        }]);
        let findings = super::storage::disk_nearly_full(&s);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Critical);
    }

    #[test]
    fn healthy_disk_is_quiet() {
        let mut s = snapshot();
        s.disks = Some(vec![DiskInfo {
            mount_point: "/".into(),
            fs_type: "ext4".into(),
            total_bytes: 100_000_000_000,
            available_bytes: 60_000_000_000,
        }]);
        assert!(super::storage::disk_nearly_full(&s).is_empty());
    }

    #[test]
    fn low_memory_severities() {
        let mut s = snapshot();
        s.memory = Some(MemoryInfo {
            total_kb: 16_000_000,
            available_kb: 640_000, // 4%
            swap_total_kb: 0,
            swap_free_kb: 0,
        });
        let findings = super::memory::low_available(&s);
        assert_eq!(findings[0].severity, Severity::Critical);
    }

    #[test]
    fn zombie_finding_carries_evidence() {
        let mut s = snapshot();
        s.processes = Some(ProcessStats {
            total: 200,
            zombies: vec!["4242 defunct-worker".into()],
            top_memory: vec![],
        });
        let findings = super::processes::zombies(&s);
        assert_eq!(findings[0].evidence, vec!["4242 defunct-worker"]);
        assert_eq!(findings[0].severity, Severity::Low);
    }

    #[test]
    fn slow_boot_thresholds() {
        let mut s = snapshot();
        s.boot = Some(BootInfo {
            total_seconds: 150.0,
            slowest_units: vec![UnitTime {
                unit: "NetworkManager-wait-online.service".into(),
                seconds: 45.0,
            }],
        });
        let findings = super::boot::slow_boot(&s);
        assert_eq!(findings[0].severity, Severity::High);
        assert!(findings[0].evidence[0].contains("NetworkManager"));
    }

    #[test]
    fn security_updates_are_high_severity() {
        let mut s = snapshot();
        s.packages = Some(PackageInfo {
            security_upgrades: Some(3),
            ..Default::default()
        });
        let findings = super::packages::security_updates(&s);
        assert_eq!(findings[0].severity, Severity::High);
    }

    #[test]
    fn ssh_root_login_only_fires_on_explicit_yes() {
        let mut s = snapshot();
        s.security = Some(SecurityInfo {
            firewall_active: Some(true),
            ssh_permit_root_login: Some(false),
            ssh_password_auth: None,
        });
        assert!(super::security::ssh_root_login(&s).is_empty());
        s.security.as_mut().unwrap().ssh_permit_root_login = Some(true);
        assert_eq!(super::security::ssh_root_login(&s).len(), 1);
    }

    #[test]
    fn every_emitted_id_is_declared() {
        // Fire every rule with a maximally unhealthy snapshot and check ids.
        let mut s = snapshot();
        s.disks = Some(vec![DiskInfo {
            mount_point: "/".into(),
            fs_type: "ext4".into(),
            total_bytes: 100,
            available_bytes: 1,
        }]);
        s.memory = Some(MemoryInfo {
            total_kb: 1000,
            available_kb: 10,
            swap_total_kb: 1000,
            swap_free_kb: 100,
        });
        s.cpu = Some(CpuInfo {
            model: "t".into(),
            logical_cores: 1,
            load_1: 9.0,
            load_5: 9.0,
            load_15: 9.0,
        });
        s.thermal = Some(ThermalInfo {
            sensors: vec![ThermalSensor {
                name: "x86_pkg_temp".into(),
                temp_c: 99.0,
            }],
        });
        s.processes = Some(ProcessStats {
            total: 10,
            zombies: vec!["1 z".into()],
            top_memory: vec![],
        });
        s.services = Some(ServiceStats {
            running: 10,
            failed: vec!["broken.service".into()],
        });
        s.boot = Some(BootInfo {
            total_seconds: 500.0,
            slowest_units: vec![],
        });
        s.packages = Some(PackageInfo {
            broken: vec!["libfoo".into()],
            old_kernels: vec!["a".into(), "b".into(), "c".into()],
            apt_cache_bytes: Some(2 * 1024 * 1024 * 1024),
            upgradable: Some(50),
            security_upgrades: Some(2),
        });
        s.logs = Some(LogInfo {
            journal_bytes: Some(5 * 1024 * 1024 * 1024),
            large_files: vec![LargeFile {
                path: "/var/log/huge.log".into(),
                bytes: 2 * 1024 * 1024 * 1024,
            }],
        });
        s.snap = Some(SnapInfo {
            disabled_revisions: 3,
            snaps_dir_bytes: None,
        });
        s.flatpak = Some(FlatpakInfo {
            unused_refs: vec!["runtime/org.freedesktop.Platform/x86_64/23.08".into()],
        });
        s.battery = Some(BatteryInfo {
            capacity_percent: Some(50),
            health_percent: Some(35.0),
        });
        s.network = Some(NetworkInfo {
            has_default_route: false,
            dns_servers: vec![],
        });
        s.security = Some(SecurityInfo {
            firewall_active: Some(false),
            ssh_permit_root_login: Some(true),
            ssh_password_auth: Some(true),
        });
        s.smart = Some(vec![SmartDevice {
            device: "/dev/sda".into(),
            model: "Test SSD".into(),
            health_passed: Some(false),
            temperature_c: Some(40),
            reallocated_sectors: Some(60),
            wear_percent: Some(95),
            power_on_hours: Some(1000),
        }]);
        s.ports = Some(vec![ListeningPort {
            proto: "tcp",
            address: "0.0.0.0".into(),
            port: 22,
            exposed: true,
        }]);

        let mut fired: Vec<String> = vec![];
        for rule in crate::default_diagnostics() {
            for finding in rule.evaluate(&s) {
                assert!(
                    crate::FINDING_IDS.contains(&finding.id.as_str()),
                    "undeclared finding id {}",
                    finding.id
                );
                fired.push(finding.id);
            }
        }
        // The unhealthy snapshot must trip every declared rule.
        for id in crate::FINDING_IDS {
            assert!(fired.iter().any(|f| f == id), "rule for {id} never fired");
        }
    }
}
