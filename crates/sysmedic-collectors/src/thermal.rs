use std::fs;

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

fn read_thermal_zones() -> Vec<ThermalSensor> {
    let Ok(entries) = fs::read_dir("/sys/class/thermal") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("thermal_zone"))
        .filter_map(|e| {
            let name = util::read_trimmed(e.path().join("type"))?;
            let milli: f64 = util::read_trimmed(e.path().join("temp"))?.parse().ok()?;
            Some(ThermalSensor {
                name,
                temp_c: milli / 1000.0,
            })
        })
        .collect()
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
            if fname.starts_with("temp") && fname.ends_with("_input") {
                if let Some(milli) =
                    util::read_trimmed(file.path()).and_then(|v| v.parse::<f64>().ok())
                {
                    sensors.push(ThermalSensor {
                        name: format!("{chip}/{}", fname.trim_end_matches("_input")),
                        temp_c: milli / 1000.0,
                    });
                }
            }
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
}
