//! Centralized health thresholds.
//!
//! These constants are the single source of truth for every numeric limit
//! used by the proactive alerts ([`crate::alert`]), the diagnostic rules
//! (`sysmedic-diagnostics`) and fix applicability (`sysmedic-fixes`). Keeping
//! them here prevents the layers from drifting apart — previously a value
//! tuned in one place silently disagreed with another (e.g. a notification
//! firing while the checkup showed nothing, or a fix re-hardcoding the
//! journal limit independently of the rule that suggests it).

/// Disk usage (percent of capacity used).
pub mod disk {
    /// At/above this, a `storage.disk_nearly_full` finding is raised (Medium).
    pub const FINDING_PCT: f64 = 85.0;
    /// At/above this, a proactive "disk almost full" alert is raised (High).
    pub const ALERT_PCT: f64 = 90.0;
    /// At/above this, both the finding and the alert become Critical.
    pub const CRITICAL_PCT: f64 = 95.0;
}

/// Available memory (percent of RAM free).
pub mod memory {
    /// Below this, memory pressure is flagged (High, and a proactive alert).
    pub const LOW_PCT: f64 = 10.0;
    /// Below this, it is Critical.
    pub const CRITICAL_PCT: f64 = 5.0;
}

/// Temperature (°C) of the hottest sensor.
pub mod thermal {
    /// At/above this, overheating is flagged High (and a proactive alert).
    pub const HIGH_C: f64 = 85.0;
    /// At/above this, it is Critical.
    pub const CRITICAL_C: f64 = 95.0;
}

/// Boot time (seconds, from `systemd-analyze time`).
pub mod boot {
    /// Above this, `boot.slow` is raised Medium.
    pub const SLOW_S: f64 = 60.0;
    /// Above this, it is High.
    pub const VERY_SLOW_S: f64 = 120.0;
}

/// systemd journal size on disk.
pub mod journal {
    /// At/above this, `logs.journal_large` is raised Medium — and the
    /// `fix.journal_vacuum` fix becomes applicable (same constant, so the
    /// finding and its fix can never disagree about when trimming makes sense).
    pub const LARGE_BYTES: u64 = 1024 * 1024 * 1024;
    /// At/above this, the finding is High.
    pub const HUGE_BYTES: u64 = 4 * LARGE_BYTES;
}

/// Battery health (current full-charge capacity as % of design capacity).
pub mod battery {
    /// Below this, `battery.degraded` is raised Medium.
    pub const DEGRADED_PCT: f64 = 60.0;
    /// Below this, it is High.
    pub const WORN_OUT_PCT: f64 = 40.0;
}

/// SMART disk-health indicators.
pub mod smart {
    /// Reallocated sectors at/above this make the finding High (any nonzero
    /// count already raises a Medium finding).
    pub const REALLOCATED_HIGH: u64 = 50;
    /// NVMe `percentage_used` at/above this raises `smart.ssd_wear` (Medium).
    pub const WEAR_PCT: u64 = 80;
    /// At/above this (rated life fully used), the wear finding is High.
    pub const WEAR_HIGH_PCT: u64 = 100;
}

/// Package hygiene.
pub mod packages {
    /// More pending updates than this raises `packages.upgrades_pending`.
    pub const UPGRADABLE_BACKLOG: u32 = 20;
    /// A package index older than this many days raises
    /// `security.index_stale`. Debian/Ubuntu refresh daily by default and
    /// security advisories are published continuously, so a week without a
    /// refresh means the pending-security-update count is no longer evidence
    /// of anything.
    pub const INDEX_STALE_DAYS: u64 = 7;
    /// Old kernels beyond this many raise a finding and make
    /// `fix.autoremove` applicable (the running kernel + one fallback stay).
    pub const OLD_KERNELS_KEPT: usize = 2;
    /// APT cache size that raises `packages.apt_cache_large` (Low).
    pub const APT_CACHE_LARGE_BYTES: u64 = 1024 * 1024 * 1024;
    /// APT cache size from which `fix.apt_clean` is worth offering —
    /// deliberately lower than the finding threshold, so the fix is available
    /// before the cache becomes report-worthy.
    pub const APT_CACHE_FIX_BYTES: u64 = 100 * 1024 * 1024;
    /// pacman cache size that raises `packages.pacman_cache_large` (Low).
    /// Higher than the APT threshold: pacman deliberately keeps every
    /// downloaded version, so a couple of GiB is normal on Arch.
    pub const PACMAN_CACHE_LARGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
}
