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

/// The unit name from a `systemctl --plain --no-legend` line.
///
/// A failed/non-good unit is prefixed with a status marker that systemd emits
/// as its **own** column: `●` (U+25CF), `×` (U+00D7, systemd ≥ 253), or `*`.
/// We therefore take the first token that is not a marker, rather than trimming
/// leading characters off the first token — the latter both lost the unit name
/// entirely when the marker was a separate token, and mangled legitimate names
/// like `xrdp.service` by stripping a leading `x`.
pub fn parse_unit_names(s: &str) -> Vec<String> {
    const MARKERS: [&str; 3] = ["●", "×", "*"];
    s.lines()
        .filter_map(|line| {
            line.split_whitespace()
                .find(|tok| !MARKERS.contains(tok))
                .map(String::from)
        })
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

    #[test]
    fn handles_status_marker_as_a_separate_column() {
        // systemd ≥ 253 prints "× unit.service loaded failed ..." with the
        // marker in its own column; older versions use "●".
        let fixture = "× nginx.service loaded failed failed A high performance web server\n● redis.service loaded failed failed Advanced key-value store\n";
        assert_eq!(
            parse_unit_names(fixture),
            vec!["nginx.service", "redis.service"]
        );
    }

    #[test]
    fn does_not_mangle_units_starting_with_x() {
        let fixture = "xrdp.service loaded failed failed xrdp daemon\n";
        assert_eq!(parse_unit_names(fixture), vec!["xrdp.service"]);
    }
}
