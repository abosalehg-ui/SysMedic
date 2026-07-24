//! Proactive alert thresholds.
//!
//! Alerts are the subset of conditions worth interrupting the user for with a
//! desktop notification (disk nearly full, overheating, low memory, pending
//! security updates). The evaluation is pure — the CLI/daemon decide how to
//! deliver them — so the thresholds are unit-tested.

use serde::Serialize;

use crate::finding::Severity;
use crate::snapshot::Snapshot;
use crate::thresholds;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Alert {
    pub id: &'static str,
    pub urgency: Severity,
    pub title: String,
    pub body: String,
}

/// Conditions worth a proactive notification, most urgent first.
pub fn evaluate(snapshot: &Snapshot) -> Vec<Alert> {
    let mut alerts = Vec::new();

    if let Some(disks) = &snapshot.disks {
        for disk in disks {
            let used = disk.used_percent();
            if used >= thresholds::disk::ALERT_PCT {
                alerts.push(Alert {
                    id: "alert.disk_full",
                    urgency: if used >= thresholds::disk::CRITICAL_PCT {
                        Severity::Critical
                    } else {
                        Severity::High
                    },
                    title: format!("Disk almost full: {}", disk.mount_point),
                    body: format!("{used:.0}% used on {}", disk.mount_point),
                });
            }
        }
    }

    if let Some(thermal) = &snapshot.thermal {
        if let Some(hottest) = thermal.hottest() {
            if hottest.temp_c >= thresholds::thermal::HIGH_C {
                alerts.push(Alert {
                    id: "alert.overheating",
                    urgency: if hottest.temp_c >= thresholds::thermal::CRITICAL_C {
                        Severity::Critical
                    } else {
                        Severity::High
                    },
                    title: "System overheating".to_string(),
                    body: format!("{} at {:.0}°C", hottest.name, hottest.temp_c),
                });
            }
        }
    }

    if let Some(mem) = &snapshot.memory {
        let avail = mem.available_percent();
        if avail < thresholds::memory::LOW_PCT {
            alerts.push(Alert {
                id: "alert.low_memory",
                urgency: Severity::High,
                title: "Low memory".to_string(),
                body: format!("Only {avail:.0}% of RAM available"),
            });
        }
    }

    if let Some(pkgs) = &snapshot.packages {
        if let Some(n) = pkgs.security_upgrades {
            if n > 0 {
                alerts.push(Alert {
                    id: "alert.security_updates",
                    urgency: Severity::High,
                    title: format!("{n} security update(s) available"),
                    body: "Install them to stay protected.".to_string(),
                });
            }
        }
    }

    alerts.sort_by_key(|a| std::cmp::Reverse(a.urgency));
    alerts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::*;

    #[test]
    fn healthy_snapshot_has_no_alerts() {
        assert!(evaluate(&Snapshot::default()).is_empty());
    }

    #[test]
    fn full_disk_triggers_critical_alert() {
        let s = Snapshot {
            disks: Some(vec![DiskInfo {
                mount_point: "/".into(),
                fs_type: "ext4".into(),
                total_bytes: 100,
                available_bytes: 2,
            }]),
            ..Default::default()
        };
        let alerts = evaluate(&s);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].id, "alert.disk_full");
        assert_eq!(alerts[0].urgency, Severity::Critical);
    }

    #[test]
    fn alerts_sorted_by_urgency() {
        let s = Snapshot {
            disks: Some(vec![DiskInfo {
                mount_point: "/".into(),
                fs_type: "ext4".into(),
                total_bytes: 100,
                available_bytes: 8, // 92% -> High
            }]),
            thermal: Some(ThermalInfo {
                sensors: vec![ThermalSensor {
                    name: "pkg".into(),
                    temp_c: 99.0, // Critical
                }],
            }),
            ..Default::default()
        };
        let alerts = evaluate(&s);
        assert_eq!(alerts.len(), 2);
        // Critical overheating sorts before the High disk alert.
        assert_eq!(alerts[0].id, "alert.overheating");
    }
}
