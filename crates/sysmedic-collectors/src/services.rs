use sysmedic_core::snapshot::ServiceStats;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct ServiceCollector;

impl Collector for ServiceCollector {
    fn name(&self) -> &'static str {
        "services"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let failed = util::run(
            "systemctl",
            &["--failed", "--plain", "--no-legend", "--no-pager"],
        );
        let running = util::run(
            "systemctl",
            &[
                "list-units",
                "--type=service",
                "--state=running",
                "--plain",
                "--no-legend",
                "--no-pager",
            ],
        );
        match (failed, running) {
            (Some(f), Some(r)) => {
                snapshot.services = Some(ServiceStats {
                    running: parse_unit_names(&r).len() as u32,
                    failed: parse_unit_names(&f),
                });
            }
            _ => snapshot
                .collection_errors
                .push("services: systemctl unavailable (not a systemd system?)".into()),
        }
    }
}

/// First column (unit name) of `systemctl --plain --no-legend` output.
pub fn parse_unit_names(s: &str) -> Vec<String> {
    s.lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|u| u.trim_start_matches(['●', '*', 'x']).to_string())
        .filter(|u| !u.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_failed_units() {
        let fixture = "fwupd-refresh.service loaded failed failed Refresh fwupd metadata\nsnap.lxd.activate.service loaded failed failed Service for snap\n";
        let units = parse_unit_names(fixture);
        assert_eq!(
            units,
            vec!["fwupd-refresh.service", "snap.lxd.activate.service"]
        );
    }

    #[test]
    fn empty_output_means_no_units() {
        assert!(parse_unit_names("").is_empty());
    }
}
