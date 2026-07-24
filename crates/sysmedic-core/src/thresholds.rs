//! Centralized health thresholds.
//!
//! These constants are the single source of truth for the numeric limits used
//! by both the proactive alerts ([`crate::alert`]) and the diagnostic rules
//! (`sysmedic-diagnostics`). Keeping them here prevents the two from drifting
//! apart — previously a value tuned in one place silently disagreed with the
//! other (e.g. a notification firing while the checkup showed nothing).

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
