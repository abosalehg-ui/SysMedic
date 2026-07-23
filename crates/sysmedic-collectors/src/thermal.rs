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
        if sensors.is_empty() {
            // Common in VMs/containers — not an error worth surfacing.
            return;
        }
        snapshot.thermal = Some(ThermalInfo { sensors });
    }
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
