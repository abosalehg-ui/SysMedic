use std::fs;

use sysmedic_core::snapshot::{ProcessStats, ProcessTop};
use sysmedic_core::{Collector, Snapshot};

use crate::util;

const PAGE_SIZE_KB: u64 = 4; // 4096-byte pages, the norm on Linux x86/arm64

pub struct ProcessCollector;

impl Collector for ProcessCollector {
    fn name(&self) -> &'static str {
        "process"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let Ok(entries) = fs::read_dir("/proc") else {
            snapshot
                .collection_errors
                .push("process: /proc not readable".into());
            return;
        };
        let mut total = 0u32;
        let mut zombies = Vec::new();
        let mut procs: Vec<ProcessTop> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(pid) = name.parse::<u32>() else {
                continue;
            };
            total += 1;
            let dir = entry.path();
            let comm = util::read_trimmed(dir.join("comm")).unwrap_or_else(|| "?".into());
            if let Some(state) =
                util::read_file(dir.join("stat")).and_then(|s| parse_stat_state(&s))
            {
                if state == 'Z' {
                    zombies.push(format!("{pid} {comm}"));
                }
            }
            if let Some(rss_kb) =
                util::read_file(dir.join("statm")).and_then(|s| parse_statm_rss_kb(&s))
            {
                procs.push(ProcessTop {
                    pid,
                    name: comm,
                    rss_kb,
                });
            }
        }
        procs.sort_by(|a, b| b.rss_kb.cmp(&a.rss_kb));
        procs.truncate(5);
        snapshot.processes = Some(ProcessStats {
            total,
            zombies,
            top_memory: procs,
        });
    }
}

/// Extract the process state from `/proc/<pid>/stat`, robust against
/// parentheses inside the command name.
pub fn parse_stat_state(stat: &str) -> Option<char> {
    let close = stat.rfind(')')?;
    stat[close + 1..].split_whitespace().next()?.chars().next()
}

/// Resident set size in kB from `/proc/<pid>/statm` (second field, pages).
pub fn parse_statm_rss_kb(statm: &str) -> Option<u64> {
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * PAGE_SIZE_KB)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_state_with_parens_in_comm() {
        let stat = "1234 (weird) name) Z 1 1234 1234 0 -1 4194560";
        assert_eq!(parse_stat_state(stat), Some('Z'));
    }

    #[test]
    fn parses_statm_rss() {
        assert_eq!(parse_statm_rss_kb("2500 1250 300 50 0 400 0"), Some(5000));
    }
}
