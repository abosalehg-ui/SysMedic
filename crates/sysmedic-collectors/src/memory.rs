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
    let mut fields = std::collections::HashMap::new();
    for line in s.lines() {
        // Tolerate any unexpected line (vendor kernels, trailing content)
        // rather than aborting the whole parse on the first one without a colon.
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        if let Some(v) = rest
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<u64>().ok())
        {
            fields.insert(key.trim(), v);
        }
    }

    let total = *fields.get("MemTotal")?;
    // `MemAvailable` is absent on kernels < 3.14 (and some container runtimes);
    // approximate it from free memory plus reclaimable cache so those systems
    // still get a memory reading instead of a blanket parse failure.
    let available = fields.get("MemAvailable").copied().unwrap_or_else(|| {
        fields.get("MemFree").copied().unwrap_or(0)
            + fields.get("Buffers").copied().unwrap_or(0)
            + fields.get("Cached").copied().unwrap_or(0)
    });
    // Swap lines are absent when swap is disabled (common in containers).
    let swap_total = fields.get("SwapTotal").copied().unwrap_or(0);
    let swap_free = fields.get("SwapFree").copied().unwrap_or(0);

    Some(MemoryInfo {
        total_kb: total,
        available_kb: available,
        swap_total_kb: swap_total,
        swap_free_kb: swap_free,
    })
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

    #[test]
    fn falls_back_when_memavailable_absent() {
        // Old kernel: no MemAvailable, no swap lines.
        let fixture = "MemTotal:    1000 kB\nMemFree:      300 kB\nBuffers:      100 kB\nCached:       200 kB\n";
        let mem = parse_meminfo(fixture).unwrap();
        assert_eq!(mem.total_kb, 1000);
        assert_eq!(mem.available_kb, 600); // 300 + 100 + 200
        assert_eq!(mem.swap_total_kb, 0);
    }

    #[test]
    fn tolerates_a_line_without_a_colon() {
        let fixture = "some junk line\nMemTotal: 1000 kB\nMemAvailable: 500 kB\n";
        assert_eq!(parse_meminfo(fixture).unwrap().available_kb, 500);
    }

    #[test]
    fn no_memtotal_is_still_a_failure() {
        assert!(parse_meminfo("MemFree: 100 kB\n").is_none());
    }
}
