use serde::Serialize;

/// Everything the collectors observed about the system in one pass.
///
/// Every section is optional: a missing tool, denied permission or an
/// unsupported platform leaves the section `None` and diagnostics simply
/// skip it. Collection problems are recorded in `collection_errors`
/// instead of failing the whole checkup.
#[derive(Debug, Default, Clone, Serialize)]
pub struct Snapshot {
    pub cpu: Option<CpuInfo>,
    pub memory: Option<MemoryInfo>,
    pub disks: Option<Vec<DiskInfo>>,
    pub thermal: Option<ThermalInfo>,
    pub processes: Option<ProcessStats>,
    pub services: Option<ServiceStats>,
    pub packages: Option<PackageInfo>,
    pub boot: Option<BootInfo>,
    pub logs: Option<LogInfo>,
    pub network: Option<NetworkInfo>,
    pub security: Option<SecurityInfo>,
    pub battery: Option<BatteryInfo>,
    pub snap: Option<SnapInfo>,
    pub flatpak: Option<FlatpakInfo>,
    pub smart: Option<Vec<SmartDevice>>,
    pub ports: Option<Vec<ListeningPort>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collection_errors: Vec<String>,
}

impl Snapshot {
    /// Fold another (partial) snapshot into this one. The engine collects
    /// each section on its own thread into a fresh `Snapshot`, then merges
    /// them in collector order; each collector fills only its own sections,
    /// so for `Option` fields the first value set wins and error notes are
    /// concatenated.
    pub fn merge(&mut self, other: Snapshot) {
        fn keep<T>(dst: &mut Option<T>, src: Option<T>) {
            if dst.is_none() {
                *dst = src;
            }
        }
        keep(&mut self.cpu, other.cpu);
        keep(&mut self.memory, other.memory);
        keep(&mut self.disks, other.disks);
        keep(&mut self.thermal, other.thermal);
        keep(&mut self.processes, other.processes);
        keep(&mut self.services, other.services);
        keep(&mut self.packages, other.packages);
        keep(&mut self.boot, other.boot);
        keep(&mut self.logs, other.logs);
        keep(&mut self.network, other.network);
        keep(&mut self.security, other.security);
        keep(&mut self.battery, other.battery);
        keep(&mut self.snap, other.snap);
        keep(&mut self.flatpak, other.flatpak);
        keep(&mut self.smart, other.smart);
        keep(&mut self.ports, other.ports);
        self.collection_errors.extend(other.collection_errors);
    }

    /// The health categories this snapshot actually has data for.
    ///
    /// A category with no data produces no findings, and a category with no
    /// findings used to score a clean 100 — so a machine where `smartctl`,
    /// `systemd` and `ufw` were all missing graded "Excellent" on the strength
    /// of what was never measured. The score is now averaged over measured
    /// categories only and the report states the coverage, so the headline
    /// number means "of what we could see" rather than an assumed all-clear.
    ///
    /// A category counts as measured when the section its rules read is
    /// present. Storage and Security draw on more than one collector, and any
    /// one of them is enough to score the category.
    pub fn measured_categories(&self) -> Vec<crate::finding::Category> {
        use crate::finding::Category;
        let mut measured = Vec::new();
        let mut mark = |category, present: bool| {
            if present {
                measured.push(category);
            }
        };
        mark(Category::Boot, self.boot.is_some());
        mark(Category::Cpu, self.cpu.is_some());
        mark(Category::Memory, self.memory.is_some());
        mark(
            Category::Storage,
            self.disks.is_some() || self.smart.is_some() || self.snap.is_some(),
        );
        mark(Category::Thermal, self.thermal.is_some());
        mark(Category::Processes, self.processes.is_some());
        mark(Category::Services, self.services.is_some());
        mark(Category::Packages, self.packages.is_some());
        mark(Category::Logs, self.logs.is_some());
        mark(Category::Network, self.network.is_some());
        mark(
            Category::Security,
            self.security.is_some() || self.ports.is_some() || self.packages.is_some(),
        );
        mark(Category::Battery, self.battery.is_some());
        measured
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuInfo {
    pub model: String,
    pub logical_cores: u32,
    pub load_1: f64,
    pub load_5: f64,
    pub load_15: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryInfo {
    pub total_kb: u64,
    pub available_kb: u64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
}

impl MemoryInfo {
    pub fn available_percent(&self) -> f64 {
        if self.total_kb == 0 {
            return 100.0;
        }
        self.available_kb as f64 / self.total_kb as f64 * 100.0
    }

    pub fn swap_used_percent(&self) -> Option<f64> {
        if self.swap_total_kb == 0 {
            return None;
        }
        let used = self.swap_total_kb.saturating_sub(self.swap_free_kb);
        Some(used as f64 / self.swap_total_kb as f64 * 100.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub mount_point: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

impl DiskInfo {
    pub fn used_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        let used = self.total_bytes.saturating_sub(self.available_bytes);
        used as f64 / self.total_bytes as f64 * 100.0
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ThermalSensor {
    pub name: String,
    pub temp_c: f64,
    /// The sensor's own critical limit (`temp*_crit` in hwmon, the `critical`
    /// trip point of a thermal zone), when the driver publishes one.
    ///
    /// One fixed threshold for every sensor is wrong for most of them: a
    /// GPU hotspot (`amdgpu` junction) runs at 95–105 °C by design with a
    /// limit of 110, while an NVMe drive is in trouble well before 95. The
    /// old 85/95 °C cut-offs turned a gaming laptop under load into a
    /// Critical "overheating" finding that capped the whole score at 59 and
    /// sent a critical desktop notification on every scheduled run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crit_c: Option<f64>,
    /// The sensor's warning limit (`temp*_max`), when published and below
    /// [`ThermalSensor::crit_c`]; NVMe exposes its vendor warning threshold
    /// here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_c: Option<f64>,
}

impl ThermalSensor {
    /// How worrying this reading is, judged against the sensor's own limits
    /// when it has them and against the fixed thresholds in
    /// [`crate::thresholds::thermal`] otherwise.
    ///
    /// With a critical limit: `High` from [`crate::thresholds::thermal::HIGH_MARGIN_C`]
    /// below it (or from the warning limit, when that is lower), `Critical`
    /// at the limit itself.
    pub fn severity(&self) -> Option<crate::finding::Severity> {
        use crate::finding::Severity;
        let (high, critical) = self.limits();
        if self.temp_c >= critical {
            Some(Severity::Critical)
        } else if self.temp_c >= high {
            Some(Severity::High)
        } else {
            None
        }
    }

    /// The (high, critical) thresholds that apply to this sensor.
    pub fn limits(&self) -> (f64, f64) {
        use crate::thresholds::thermal as th;
        let plausible = |v: Option<f64>| v.filter(|&v| v > 0.0 && v < 200.0);
        match (plausible(self.max_c), plausible(self.crit_c)) {
            (max, Some(crit)) => {
                let high = match max {
                    Some(max) if max < crit => max,
                    _ => crit - th::HIGH_MARGIN_C,
                };
                (high, crit)
            }
            // Only a warning limit: that is where High starts; Critical stays
            // at the fixed cut-off (or just above the warning if that is
            // higher still).
            (Some(max), None) => (max, max.max(th::CRITICAL_C)),
            (None, None) => (th::HIGH_C, th::CRITICAL_C),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ThermalInfo {
    pub sensors: Vec<ThermalSensor>,
}

impl ThermalInfo {
    pub fn hottest(&self) -> Option<&ThermalSensor> {
        self.sensors
            .iter()
            .max_by(|a, b| a.temp_c.total_cmp(&b.temp_c))
    }

    /// The sensor in the worst state, judged by its own limits, with that
    /// state. `None` when every sensor is within its limits.
    ///
    /// Replaces "the hottest sensor against one fixed threshold": the
    /// hottest reading on a machine is routinely a GPU hotspot that is
    /// nowhere near its limit, while a cooler NVMe drive may be past its own.
    pub fn worst(&self) -> Option<(&ThermalSensor, crate::finding::Severity)> {
        self.sensors
            .iter()
            .filter_map(|s| s.severity().map(|sev| (s, sev)))
            .max_by(|a, b| {
                a.1.cmp(&b.1)
                    .then_with(|| a.0.temp_c.total_cmp(&b.0.temp_c))
            })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessTop {
    pub pid: u32,
    pub name: String,
    pub rss_kb: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessStats {
    pub total: u32,
    /// `"pid name"` of each zombie process.
    pub zombies: Vec<String>,
    pub top_memory: Vec<ProcessTop>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStats {
    pub running: u32,
    pub failed: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct PackageInfo {
    /// Which package manager produced this section: `"apt"`, `"dnf"` or
    /// `"pacman"`. Lets diagnostics phrase remedies for the right tool and
    /// keeps apt-specific fixes from being offered elsewhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manager: Option<String>,
    pub broken: Vec<String>,
    /// Installed kernel image packages other than the running kernel
    /// (collected on Debian-family systems only).
    pub old_kernels: Vec<String>,
    /// Size of the APT download cache (apt systems only — gates the
    /// apt-specific `fix.apt_clean`).
    pub apt_cache_bytes: Option<u64>,
    /// Size of /var/cache/pacman/pkg (pacman systems only). pacman keeps
    /// every downloaded package version until the cache is cleaned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pacman_cache_bytes: Option<u64>,
    pub upgradable: Option<u32>,
    pub security_upgrades: Option<u32>,
    /// Packages `autoremove --purge` would delete, from a simulation run.
    ///
    /// The fix preview is the consent contract for a privileged, irreversible
    /// change, so it must name what will actually be removed rather than the
    /// old kernels alone. Collected once per checkup here (the collectors own
    /// the I/O) so building the plan stays pure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub autoremovable: Vec<String>,
    /// Age in days of the package index (`apt update` metadata).
    ///
    /// `security_upgrades` is only as truthful as the index it was computed
    /// from: on a machine that has not refreshed in months, "0 security
    /// updates pending" is an absence of knowledge reported as an all-clear.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_age_days: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnitTime {
    pub unit: String,
    pub seconds: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BootInfo {
    pub total_seconds: f64,
    pub slowest_units: Vec<UnitTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LargeFile {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct LogInfo {
    pub journal_bytes: Option<u64>,
    pub large_files: Vec<LargeFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkInfo {
    pub has_default_route: bool,
    pub dns_servers: Vec<String>,
}

/// Which firewall front-end the machine actually runs.
///
/// [`SecurityInfo::firewall_active`] says *whether* a firewall is on, never
/// *which* one. That was enough while ufw was the only front-end understood,
/// but once firewalld could answer too, a `Some(false)` on a Fedora/RHEL
/// machine put ufw-specific rule text on screen and offered `fix.enable_ufw`
/// on a system with no `ufw` binary — a fix the user authorized with their
/// password and which could only fail inside the privileged helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FirewallFrontend {
    Ufw,
    Firewalld,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct SecurityInfo {
    /// `None` when no known firewall frontend could be queried.
    pub firewall_active: Option<bool>,
    /// The front-end [`SecurityInfo::firewall_active`] describes. `None` means
    /// no front-end SysMedic understands is installed (a hand-rolled nftables
    /// ruleset, say), in which case no firewall finding or fix applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firewall_frontend: Option<FirewallFrontend>,
    /// `None` when sshd is not installed / config unreadable.
    pub ssh_permit_root_login: Option<bool>,
    /// Effective `PasswordAuthentication`; `None` when sshd absent/unreadable.
    pub ssh_password_auth: Option<bool>,
    /// The TCP ports sshd is configured to listen on (every `Port` directive,
    /// or 22 when none is set). Empty when no `sshd_config` could be read,
    /// which means no SSH server is installed.
    ///
    /// Needed because "SSH reachable" used to mean "something listens on 22":
    /// moving sshd to another port is one of the most common hardening
    /// steps, and a firewall fix that only allowed 22 locked those machines
    /// out while its preview promised the opposite.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ssh_ports: Vec<u16>,
}

/// One disk's SMART health, as reported by `smartctl`.
#[derive(Debug, Clone, Serialize)]
pub struct SmartDevice {
    pub device: String,
    pub model: String,
    /// Overall SMART self-assessment; `None` if the drive didn't report it.
    pub health_passed: Option<bool>,
    pub temperature_c: Option<i64>,
    /// ATA "Reallocated_Sector_Ct" raw value (bad sectors remapped).
    pub reallocated_sectors: Option<u64>,
    /// NVMe "percentage_used" wear indicator (0–100+, 100 = rated life used).
    pub wear_percent: Option<u64>,
    pub power_on_hours: Option<u64>,
}

/// A socket in the LISTEN state, from `/proc/net/tcp{,6}`.
#[derive(Debug, Clone, Serialize)]
pub struct ListeningPort {
    pub proto: &'static str,
    pub address: String,
    pub port: u16,
    /// True when bound to a wildcard/all-interfaces address (0.0.0.0 or ::),
    /// i.e. reachable from the network rather than only localhost.
    pub exposed: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct BatteryInfo {
    pub capacity_percent: Option<u32>,
    /// Current full-charge capacity relative to design capacity.
    pub health_percent: Option<f64>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct SnapInfo {
    /// Every disabled (old) revision kept on disk, across all snaps.
    pub disabled_revisions: u32,
    /// Disabled revisions beyond the one rollback copy per snap that
    /// `refresh.retain=2` — the value `fix.snap_retain` sets — keeps anyway.
    ///
    /// This is what the finding and the fix are about. Counting every
    /// disabled revision meant the finding never cleared and the fix stayed
    /// on offer forever: with retain=2 each refreshed snap still holds one
    /// disabled revision, so `disabled_revisions > 0` was true on every
    /// Ubuntu machine, including right after the fix had been applied.
    #[serde(default)]
    pub excess_revisions: u32,
    pub snaps_dir_bytes: Option<u64>,
    /// The system's current `refresh.retain` setting, so undoing
    /// `fix.snap_retain` can restore the user's value instead of assuming
    /// snapd's default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_retain: Option<u32>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct FlatpakInfo {
    /// Runtimes/extensions no application depends on any more.
    pub unused_refs: Vec<String>,
}
