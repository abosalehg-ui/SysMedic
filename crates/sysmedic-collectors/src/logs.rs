use std::fs;
use std::path::Path;

use sysmedic_core::snapshot::{LargeFile, LogInfo};
use sysmedic_core::{Collector, Snapshot};

use crate::util;

const LARGE_LOG_BYTES: u64 = 100 * 1024 * 1024;

pub struct LogCollector;

impl Collector for LogCollector {
    fn name(&self) -> &'static str {
        "logs"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let mut info = LogInfo {
            journal_bytes: util::run("journalctl", &["--disk-usage"])
                .and_then(|s| parse_journal_usage(&s)),
            large_files: Vec::new(),
        };
        find_large_files(Path::new("/var/log"), 3, &mut info.large_files);
        info.large_files.sort_by_key(|f| std::cmp::Reverse(f.bytes));
        info.large_files.truncate(10);
        snapshot.logs = Some(info);
    }
}

/// Size out of `Archived and active journals take up 3.9G in the file system.`
pub fn parse_journal_usage(s: &str) -> Option<u64> {
    let idx = s.find("take up")?;
    let token = s[idx + "take up".len()..].split_whitespace().next()?;
    util::parse_size(token)
}

fn find_large_files(dir: &Path, depth: u32, out: &mut Vec<LargeFile>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_file() && meta.len() >= LARGE_LOG_BYTES {
            out.push(LargeFile {
                path: entry.path().display().to_string(),
                bytes: meta.len(),
            });
        } else if meta.is_dir() && depth > 0 {
            find_large_files(&entry.path(), depth - 1, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_journal_usage() {
        let fixture = "Archived and active journals take up 3.9G in the file system.\n";
        assert_eq!(
            parse_journal_usage(fixture),
            Some((3.9 * 1024.0 * 1024.0 * 1024.0) as u64)
        );
    }
}
