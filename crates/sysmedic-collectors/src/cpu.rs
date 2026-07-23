use sysmedic_core::snapshot::CpuInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct CpuCollector;

impl Collector for CpuCollector {
    fn name(&self) -> &'static str {
        "cpu"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let loadavg = util::read_file("/proc/loadavg").and_then(|s| parse_loadavg(&s));
        let cpuinfo = util::read_file("/proc/cpuinfo").map(|s| parse_cpuinfo(&s));
        match (loadavg, cpuinfo) {
            (Some((l1, l5, l15)), Some((model, cores))) => {
                snapshot.cpu = Some(CpuInfo {
                    model,
                    logical_cores: cores,
                    load_1: l1,
                    load_5: l5,
                    load_15: l15,
                });
            }
            _ => snapshot
                .collection_errors
                .push("cpu: could not read /proc/loadavg or /proc/cpuinfo".into()),
        }
    }
}

pub fn parse_loadavg(s: &str) -> Option<(f64, f64, f64)> {
    let mut it = s.split_whitespace();
    Some((
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    ))
}

pub fn parse_cpuinfo(s: &str) -> (String, u32) {
    let mut model = String::from("unknown");
    let mut cores = 0u32;
    for line in s.lines() {
        if line.starts_with("processor") {
            cores += 1;
        } else if model == "unknown" && line.starts_with("model name") {
            if let Some((_, v)) = line.split_once(':') {
                model = v.trim().to_string();
            }
        }
    }
    (model, cores.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_loadavg() {
        let (l1, l5, l15) = parse_loadavg("0.52 0.58 0.59 1/389 12345\n").unwrap();
        assert_eq!((l1, l5, l15), (0.52, 0.58, 0.59));
    }

    #[test]
    fn parses_cpuinfo() {
        let fixture = "processor\t: 0\nmodel name\t: AMD Ryzen 7 5800X\nprocessor\t: 1\nmodel name\t: AMD Ryzen 7 5800X\n";
        let (model, cores) = parse_cpuinfo(fixture);
        assert_eq!(model, "AMD Ryzen 7 5800X");
        assert_eq!(cores, 2);
    }
}
