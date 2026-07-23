use sysmedic_core::snapshot::MemoryInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct MemoryCollector;

impl Collector for MemoryCollector {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        match util::read_file("/proc/meminfo").and_then(|s| parse_meminfo(&s)) {
            Some(mem) => snapshot.memory = Some(mem),
            None => snapshot
                .collection_errors
                .push("memory: could not parse /proc/meminfo".into()),
        }
    }
}

pub fn parse_meminfo(s: &str) -> Option<MemoryInfo> {
    let mut total = None;
    let mut available = None;
    let mut swap_total = None;
    let mut swap_free = None;
    for line in s.lines() {
        let (key, rest) = line.split_once(':')?;
        let value_kb: Option<u64> = rest.split_whitespace().next().and_then(|v| v.parse().ok());
        match key {
            "MemTotal" => total = value_kb,
            "MemAvailable" => available = value_kb,
            "SwapTotal" => swap_total = value_kb,
            "SwapFree" => swap_free = value_kb,
            _ => {}
        }
        if let (Some(t), Some(a), Some(st), Some(sf)) = (total, available, swap_total, swap_free) {
            return Some(MemoryInfo {
                total_kb: t,
                available_kb: a,
                swap_total_kb: st,
                swap_free_kb: sf,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo() {
        let fixture = "MemTotal:       16303428 kB\nMemFree:         1234567 kB\nMemAvailable:    8151714 kB\nBuffers:          200000 kB\nSwapTotal:       2097148 kB\nSwapFree:        2097148 kB\n";
        let mem = parse_meminfo(fixture).unwrap();
        assert_eq!(mem.total_kb, 16_303_428);
        assert_eq!(mem.available_kb, 8_151_714);
        assert!((mem.available_percent() - 50.0).abs() < 0.1);
        assert_eq!(mem.swap_used_percent(), Some(0.0));
    }
}
