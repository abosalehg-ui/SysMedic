//! Proactive alert thresholds.
//!
//! Alerts are the subset of conditions worth interrupting the user for with a
//! desktop notification (disk nearly full, overheating, low memory, pending
//! security updates). The evaluation is pure — the CLI/daemon decide how to
//! deliver them — so the thresholds are unit-tested.

use serde::Serialize;

use crate::finding::Severity;
use crate::lang::Lang;
use crate::snapshot::Snapshot;
use crate::thresholds;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Alert {
    pub id: &'static str,
    pub urgency: Severity,
    pub title: String,
    pub body: String,
}

/// Conditions worth a proactive notification, most urgent first, with their
/// text in `lang`.
///
/// A desktop notification is the *only* part of the follow-up flow a user sees
/// when they are not looking at SysMedic, so shipping it English-only left the
/// one proactive touchpoint untranslated on an Arabic system.
pub fn evaluate_in(snapshot: &Snapshot, lang: Lang) -> Vec<Alert> {
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
                    title: match lang {
                        Lang::En => format!("Disk almost full: {}", disk.mount_point),
                        Lang::Ar => format!("القرص شارف على الامتلاء: {}", disk.mount_point),
                    },
                    body: match lang {
                        Lang::En => format!("{used:.0}% used on {}", disk.mount_point),
                        Lang::Ar => {
                            format!("استُخدم {used:.0}% من {}", disk.mount_point)
                        }
                    },
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
                    title: match lang {
                        Lang::En => "System overheating".to_string(),
                        Lang::Ar => "ارتفاع حرارة النظام".to_string(),
                    },
                    body: match lang {
                        Lang::En => format!("{} at {:.0}°C", hottest.name, hottest.temp_c),
                        Lang::Ar => {
                            format!("{} عند {:.0}°م", hottest.name, hottest.temp_c)
                        }
                    },
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
                title: match lang {
                    Lang::En => "Low memory".to_string(),
                    Lang::Ar => "الذاكرة منخفضة".to_string(),
                },
                body: match lang {
                    Lang::En => format!("Only {avail:.0}% of RAM available"),
                    Lang::Ar => format!("لم يتبقَّ سوى {avail:.0}% من الذاكرة"),
                },
            });
        }
    }

    if let Some(pkgs) = &snapshot.packages {
        if let Some(n) = pkgs.security_upgrades {
            if n > 0 {
                alerts.push(Alert {
                    id: "alert.security_updates",
                    urgency: Severity::High,
                    title: match lang {
                        Lang::En => format!("{n} security update(s) available"),
                        Lang::Ar => format!("يتوفّر {n} تحديث أمني"),
                    },
                    body: match lang {
                        Lang::En => "Install them to stay protected.".to_string(),
                        Lang::Ar => "ثبّتها للحفاظ على حماية النظام.".to_string(),
                    },
                });
            }
        }
    }

    alerts.sort_by_key(|a| std::cmp::Reverse(a.urgency));
    alerts
}

/// [`evaluate_in`] in English. Kept for callers that have no locale context.
pub fn evaluate(snapshot: &Snapshot) -> Vec<Alert> {
    evaluate_in(snapshot, Lang::En)
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
    fn alert_text_is_localized() {
        let s = Snapshot {
            memory: Some(MemoryInfo {
                total_kb: 1000,
                available_kb: 10,
                swap_total_kb: 0,
                swap_free_kb: 0,
            }),
            ..Default::default()
        };
        let en = evaluate_in(&s, Lang::En);
        assert_eq!(en[0].title, "Low memory");
        let ar = evaluate_in(&s, Lang::Ar);
        assert_eq!(ar[0].title, "الذاكرة منخفضة");
        assert!(ar[0].body.contains('%'));
        // Same condition, same id — only the presentation differs.
        assert_eq!(en[0].id, ar[0].id);
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
