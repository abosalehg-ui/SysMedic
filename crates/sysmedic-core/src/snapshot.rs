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

#[derive(Debug, Clone, Serialize)]
pub struct ThermalSensor {
    pub name: String,
    pub temp_c: f64,
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
    pub broken: Vec<String>,
    /// Installed kernel image packages other than the running kernel.
    pub old_kernels: Vec<String>,
    pub apt_cache_bytes: Option<u64>,
    pub upgradable: Option<u32>,
    pub security_upgrades: Option<u32>,
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

#[derive(Debug, Default, Clone, Serialize)]
pub struct SecurityInfo {
    /// `None` when no known firewall frontend could be queried.
    pub firewall_active: Option<bool>,
    /// `None` when sshd is not installed / config unreadable.
    pub ssh_permit_root_login: Option<bool>,
    /// Effective `PasswordAuthentication`; `None` when sshd absent/unreadable.
    pub ssh_password_auth: Option<bool>,
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
    pub disabled_revisions: u32,
    pub snaps_dir_bytes: Option<u64>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct FlatpakInfo {
    /// Runtimes/extensions no application depends on any more.
    pub unused_refs: Vec<String>,
}
