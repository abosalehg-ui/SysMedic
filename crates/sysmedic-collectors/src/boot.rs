use sysmedic_core::snapshot::{BootInfo, UnitTime};
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct BootCollector;

impl Collector for BootCollector {
    fn name(&self) -> &'static str {
        "boot"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let Some(time) = util::run("systemd-analyze", &["time"]) else {
            // Containers and some VMs can't answer; skip silently.
            return;
        };
        let Some(total_seconds) = parse_analyze_time(&time) else {
            snapshot
                .collection_errors
                .push("boot: unrecognized systemd-analyze output".into());
            return;
        };
        let slowest_units = util::run("systemd-analyze", &["blame", "--no-pager"])
            .map(|out| parse_blame(&out, 5))
            .unwrap_or_default();
        snapshot.boot = Some(BootInfo {
            total_seconds,
            slowest_units,
        });
    }
}

/// Total from `systemd-analyze time`, e.g.
/// `Startup finished in 4.5s (kernel) + 30.2s (userspace) = 34.8s`.
pub fn parse_analyze_time(s: &str) -> Option<f64> {
    let line = s.lines().find(|l| l.contains("Startup finished"))?;
    let total = line.rsplit_once('=')?.1;
    // The total may itself be compound ("1min 34.8s") and may be followed
    // by a "graphical.target reached..." continuation.
    parse_duration(total.split("\n").next()?.trim())
}

/// Parse systemd durations: `5.123s`, `1min 30.2s`, `2ms`, `1h 2min`.
pub fn parse_duration(s: &str) -> Option<f64> {
    let mut total = 0.0;
    let mut matched = false;
    for part in s.split_whitespace() {
        let (mult, trimmed) = if let Some(v) = part.strip_suffix("ms") {
            (0.001, v)
        } else if let Some(v) = part.strip_suffix("min") {
            (60.0, v)
        } else if let Some(v) = part.strip_suffix('h') {
            (3600.0, v)
        } else {
            (1.0, part.strip_suffix('s')?)
        };
        total += trimmed.parse::<f64>().ok()? * mult;
        matched = true;
    }
    matched.then_some(total)
}

/// Top `limit` entries of `systemd-analyze blame` (`<duration> <unit>`).
pub fn parse_blame(s: &str, limit: usize) -> Vec<UnitTime> {
    s.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (dur, unit) = line.rsplit_once(' ')?;
            Some(UnitTime {
                unit: unit.to_string(),
                seconds: parse_duration(dur.trim())?,
            })
        })
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_total() {
        let fixture = "Startup finished in 4.257s (kernel) + 28.541s (userspace) = 32.798s\ngraphical.target reached after 28.5s in userspace.\n";
        assert!((parse_analyze_time(fixture).unwrap() - 32.798).abs() < 0.001);
    }

    #[test]
    fn parses_compound_total() {
        let fixture = "Startup finished in 5.0s (kernel) + 1min 35.774s (userspace) = 1min 40.774s";
        assert!((parse_analyze_time(fixture).unwrap() - 100.774).abs() < 0.001);
    }

    #[test]
    fn parses_blame() {
        let fixture = "  32.875s NetworkManager-wait-online.service\n   1min 2.1s apt-daily.service\n   543ms dev-loop1.device\n";
        let blame = parse_blame(fixture, 5);
        assert_eq!(blame.len(), 3);
        assert_eq!(blame[0].unit, "NetworkManager-wait-online.service");
        assert!((blame[1].seconds - 62.1).abs() < 0.001);
    }
}
