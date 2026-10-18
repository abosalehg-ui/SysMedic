use std::fs;
use std::path::Path;

use sysmedic_core::snapshot::{ThermalInfo, ThermalSensor};
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct ThermalCollector;

impl Collector for ThermalCollector {
    fn name(&self) -> &'static str {
        "thermal"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let mut sensors = read_thermal_zones();
        sensors.extend(read_hwmon());
        // Drop implausible readings. Disconnected or bogus sensors on consumer
        // boards commonly report -40, 0, or a stuck 127 °C; without this filter
        // a single stuck sensor produces a false Critical overheating finding
        // and a repeated desktop alert.
        sensors.retain(|s| plausible(s.temp_c));
        if sensors.is_empty() {
            // Common in VMs/containers — not an error worth surfacing.
            return;
        }
        snapshot.thermal = Some(ThermalInfo { sensors });
    }
}

/// The range of temperatures (°C) treated as real sensor readings.
const PLAUSIBLE_C: std::ops::RangeInclusive<f64> = 1.0..=120.0;

/// Whether a reading looks like a genuine temperature rather than a
/// disconnected/stuck sensor (-40, 0, 127 °C are the common bogus values).
fn plausible(temp_c: f64) -> bool {
    PLAUSIBLE_C.contains(&temp_c)
}

/// A millidegree sysfs value as °C.
fn read_millidegrees(path: impl AsRef<Path>) -> Option<f64> {
    util::read_trimmed(path)
        .and_then(|v| v.parse::<f64>().ok())
        .map(|milli| milli / 1000.0)
}

fn read_thermal_zones() -> Vec<ThermalSensor> {
    let Ok(entries) = fs::read_dir("/sys/class/thermal") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("thermal_zone"))
        .filter_map(|e| {
            let name = util::read_trimmed(e.path().join("type"))?;
            let temp_c = read_millidegrees(e.path().join("temp"))?;
            let (max_c, crit_c) = zone_trip_points(&e.path());
            Some(ThermalSensor {
                name,
                temp_c,
                crit_c,
                max_c,
            })
        })
        .collect()
}

/// The (hot, critical) trip points of a thermal zone, from its
/// `trip_point_<n>_type` / `trip_point_<n>_temp` pairs. The kernel names the
/// shutdown threshold `critical` and the throttling one `hot` (or `passive`).
fn zone_trip_points(zone: &Path) -> (Option<f64>, Option<f64>) {
    let mut hot = None;
    let mut critical = None;
    for n in 0..16 {
        let Some(kind) = util::read_trimmed(zone.join(format!("trip_point_{n}_type"))) else {
            break;
        };
        let temp = read_millidegrees(zone.join(format!("trip_point_{n}_temp")));
        match kind.as_str() {
            "critical" => critical = critical.or(temp),
            "hot" | "passive" => hot = hot.or(temp),
            _ => {}
        }
    }
    (hot, critical)
}

fn read_hwmon() -> Vec<ThermalSensor> {
    let Ok(entries) = fs::read_dir("/sys/class/hwmon") else {
        return Vec::new();
    };
    let mut sensors = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        let Some(chip) = util::read_trimmed(dir.join("name")) else {
            continue;
        };
        let Ok(files) = fs::read_dir(&dir) else {
            continue;
        };
        for file in files.flatten() {
            let fname = file.file_name().to_string_lossy().into_owned();
            let Some(stem) = fname
                .strip_suffix("_input")
                .filter(|s| s.starts_with("temp"))
            else {
                continue;
            };
            let Some(temp_c) = read_millidegrees(file.path()) else {
                continue;
            };
            // The driver's own limits for this reading, when it publishes
            // them: `temp2_crit` for `temp2_input`, and so on.
            sensors.push(ThermalSensor {
                name: format!("{chip}/{stem}"),
                temp_c,
                crit_c: read_millidegrees(dir.join(format!("{stem}_crit"))),
                max_c: read_millidegrees(dir.join(format!("{stem}_max"))),
            });
        }
    }
    sensors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_stuck_and_disconnected_sensor_values() {
        assert!(plausible(45.0));
        assert!(plausible(95.0));
        assert!(!plausible(127.0)); // stuck sensor
        assert!(!plausible(0.0)); // disconnected
        assert!(!plausible(-40.0)); // bogus
    }

    #[test]
    fn reads_a_sensor_with_its_own_limits_from_a_hwmon_like_directory() {
        // A fake hwmon chip laid out the way the kernel does it: the limits
        // sit next to the reading under the same `tempN_` stem.
        let dir = tempfile::tempdir().unwrap();
        let chip = dir.path();
        fs::write(chip.join("temp2_input"), "100000\n").unwrap();
        fs::write(chip.join("temp2_crit"), "110000\n").unwrap();
        fs::write(chip.join("temp2_label"), "junction\n").unwrap();

        let stem = "temp2";
        let sensor = ThermalSensor {
            name: format!("amdgpu/{stem}"),
            temp_c: read_millidegrees(chip.join(format!("{stem}_input"))).unwrap(),
            crit_c: read_millidegrees(chip.join(format!("{stem}_crit"))),
            max_c: read_millidegrees(chip.join(format!("{stem}_max"))),
        };
        assert_eq!(sensor.temp_c, 100.0);
        assert_eq!(sensor.crit_c, Some(110.0));
        assert_eq!(sensor.max_c, None);
        // 100 °C on a part whose limit is 110 is High, not the Critical the
        // fixed 95 °C cut-off produced.
        assert_eq!(sensor.severity(), Some(sysmedic_core::Severity::High));
    }

    #[test]
    fn zone_trip_points_pick_hot_and_critical() {
        let dir = tempfile::tempdir().unwrap();
        let zone = dir.path();
        fs::write(zone.join("trip_point_0_type"), "critical\n").unwrap();
        fs::write(zone.join("trip_point_0_temp"), "105000\n").unwrap();
        fs::write(zone.join("trip_point_1_type"), "passive\n").unwrap();
        fs::write(zone.join("trip_point_1_temp"), "95000\n").unwrap();
        assert_eq!(zone_trip_points(zone), (Some(95.0), Some(105.0)));
        // A zone without trip points yields nothing, not a panic.
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(zone_trip_points(empty.path()), (None, None));
    }
}
