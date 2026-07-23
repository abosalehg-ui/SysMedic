use serde_json::Value;
use sysmedic_core::snapshot::SmartDevice;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct SmartCollector;

impl Collector for SmartCollector {
    fn name(&self) -> &'static str {
        "smart"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        // `smartctl` needs root; without it we degrade silently (the section
        // stays None). The parsers below are what the tests exercise.
        let Some(scan) = util::run("smartctl", &["--scan", "-j"]) else {
            return; // smartmontools not installed
        };
        let devices = parse_scan(&scan);
        if devices.is_empty() {
            return;
        }
        let mut out = Vec::new();
        let mut denied = false;
        for dev in devices {
            match util::run("smartctl", &["-j", "-H", "-A", "-i", &dev]) {
                Some(json) => {
                    if let Some(parsed) = parse_device(&json) {
                        out.push(parsed);
                    }
                }
                None => denied = true,
            }
        }
        if out.is_empty() {
            if denied {
                snapshot
                    .collection_errors
                    .push("smart: smartctl needs root to read device health".into());
            }
            return;
        }
        snapshot.smart = Some(out);
    }
}

/// Device node names from `smartctl --scan -j`.
pub fn parse_scan(json: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    v["devices"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|d| d["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a single-device `smartctl -j -H -A -i` report.
pub fn parse_device(json: &str) -> Option<SmartDevice> {
    let v: Value = serde_json::from_str(json).ok()?;
    let device = v["device"]["name"].as_str().unwrap_or("?").to_string();
    let model = v["model_name"]
        .as_str()
        .or_else(|| v["model_family"].as_str())
        .unwrap_or("unknown")
        .to_string();
    let health_passed = v["smart_status"]["passed"].as_bool();
    let temperature_c = v["temperature"]["current"].as_i64();

    // ATA: find the Reallocated_Sector_Ct (id 5) raw value.
    let reallocated_sectors = v["ata_smart_attributes"]["table"]
        .as_array()
        .and_then(|table| {
            table
                .iter()
                .find(|a| a["id"].as_u64() == Some(5))
                .and_then(|a| a["raw"]["value"].as_u64())
        });

    // NVMe wear + hours live under a different key.
    let nvme = &v["nvme_smart_health_information_log"];
    let wear_percent = nvme["percentage_used"].as_u64();
    let power_on_hours = nvme["power_on_hours"]
        .as_u64()
        .or_else(|| v["power_on_time"]["hours"].as_u64());

    Some(SmartDevice {
        device,
        model,
        health_passed,
        temperature_c,
        reallocated_sectors,
        wear_percent,
        power_on_hours,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scan_list() {
        let json =
            r#"{"devices":[{"name":"/dev/sda","type":"sat"},{"name":"/dev/nvme0","type":"nvme"}]}"#;
        assert_eq!(parse_scan(json), vec!["/dev/sda", "/dev/nvme0"]);
    }

    #[test]
    fn parses_ata_device_with_reallocated_sectors() {
        let json = r#"{
            "device": {"name": "/dev/sda"},
            "model_name": "Samsung SSD 860",
            "smart_status": {"passed": true},
            "temperature": {"current": 34},
            "power_on_time": {"hours": 12000},
            "ata_smart_attributes": {"table": [
                {"id": 5, "name": "Reallocated_Sector_Ct", "raw": {"value": 8}},
                {"id": 9, "name": "Power_On_Hours", "raw": {"value": 12000}}
            ]}
        }"#;
        let d = parse_device(json).unwrap();
        assert_eq!(d.device, "/dev/sda");
        assert_eq!(d.health_passed, Some(true));
        assert_eq!(d.temperature_c, Some(34));
        assert_eq!(d.reallocated_sectors, Some(8));
        assert_eq!(d.power_on_hours, Some(12000));
        assert_eq!(d.wear_percent, None);
    }

    #[test]
    fn parses_nvme_wear() {
        let json = r#"{
            "device": {"name": "/dev/nvme0"},
            "model_name": "WD Black SN770",
            "smart_status": {"passed": false},
            "nvme_smart_health_information_log": {
                "percentage_used": 87, "temperature": 45, "power_on_hours": 9000
            }
        }"#;
        let d = parse_device(json).unwrap();
        assert_eq!(d.health_passed, Some(false));
        assert_eq!(d.wear_percent, Some(87));
        assert_eq!(d.power_on_hours, Some(9000));
    }
}
