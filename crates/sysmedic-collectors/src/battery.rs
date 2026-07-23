use std::fs;

use sysmedic_core::snapshot::BatteryInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct BatteryCollector;

impl Collector for BatteryCollector {
    fn name(&self) -> &'static str {
        "battery"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let Ok(entries) = fs::read_dir("/sys/class/power_supply") else {
            return; // desktops/VMs: no battery is not an error
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if util::read_trimmed(dir.join("type")).as_deref() != Some("Battery") {
                continue;
            }
            let capacity_percent =
                util::read_trimmed(dir.join("capacity")).and_then(|v| v.parse().ok());
            let health_percent = ["energy", "charge"].iter().find_map(|prefix| {
                let full: f64 = util::read_trimmed(dir.join(format!("{prefix}_full")))?
                    .parse()
                    .ok()?;
                let design: f64 = util::read_trimmed(dir.join(format!("{prefix}_full_design")))?
                    .parse()
                    .ok()?;
                (design > 0.0).then(|| full / design * 100.0)
            });
            snapshot.battery = Some(BatteryInfo {
                capacity_percent,
                health_percent,
            });
            return;
        }
    }
}
