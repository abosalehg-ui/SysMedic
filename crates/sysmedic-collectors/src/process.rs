use std::fs;
use std::sync::OnceLock;

use sysmedic_core::snapshot::{ProcessStats, ProcessTop};
use sysmedic_core::{Collector, Snapshot};

use crate::util;

/// The kernel page size in kB, read once at runtime. `/proc/<pid>/statm` is in
/// pages, and the page size is not always 4 KiB: RHEL/CentOS aarch64 use 64
/// KiB pages and Apple-Silicon/Asahi kernels use 16 KiB, on which a hardcoded
/// 4 would under-report RSS by 4×–16×.
fn page_size_kb() -> u64 {
    static PAGE_KB: OnceLock<u64> = OnceLock::new();
    *PAGE_KB.get_or_init(|| {
        // SAFETY: `sysconf(_SC_PAGESIZE)` has no preconditions and no
        // observable side effects; it returns the page size in bytes.
        let bytes = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if bytes > 0 {
            bytes as u64 / 1024
        } else {
            4 // implausible return; fall back to the common default
        }
    })
}

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
            if let Some(rss_kb) = util::read_file(dir.join("statm"))
                .and_then(|s| parse_statm_rss_kb(&s, page_size_kb()))
            {
                procs.push(ProcessTop {
                    pid,
                    name: comm,
                    rss_kb,
                });
            }
        }
        procs.sort_by_key(|p| std::cmp::Reverse(p.rss_kb));
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

/// Resident set size in kB from `/proc/<pid>/statm` (second field, in pages),
/// given the system `page_size_kb`.
pub fn parse_statm_rss_kb(statm: &str, page_size_kb: u64) -> Option<u64> {
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * page_size_kb)
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
        // 1250 pages × 4 KiB = 5000 kB.
        assert_eq!(
            parse_statm_rss_kb("2500 1250 300 50 0 400 0", 4),
            Some(5000)
        );
    }

    #[test]
    fn statm_rss_scales_with_page_size() {
        // Same 1250 pages on a 16 KiB-page kernel is 4× the RSS.
        assert_eq!(
            parse_statm_rss_kb("2500 1250 300 50 0 400 0", 16),
            Some(20000)
        );
    }
}
