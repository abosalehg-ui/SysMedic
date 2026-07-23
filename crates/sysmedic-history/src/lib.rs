//! Health-score history: append one line per checkup and render the trend.
//!
//! Stored as JSON Lines at `~/.local/state/sysmedic/history.jsonl` (one
//! [`HistoryEntry`] per line) so it is append-only, human-readable and cheap
//! to tail. The sparkline/trend helpers are pure and unit-tested.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sysmedic_core::finding::Severity;
use sysmedic_core::HealthReport;

/// One recorded checkup: when, the score, and how many findings by severity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryEntry {
    pub at: String,
    pub score: u8,
    pub grade: String,
    pub findings: usize,
    pub critical: usize,
    pub high: usize,
}

impl HistoryEntry {
    pub fn from_report(report: &HealthReport) -> Self {
        let count = |s: Severity| report.findings.iter().filter(|f| f.severity == s).count();
        HistoryEntry {
            at: humantime::format_rfc3339_seconds(SystemTime::now()).to_string(),
            score: report.score,
            grade: report.grade.to_string(),
            findings: report.findings.len(),
            critical: count(Severity::Critical),
            high: count(Severity::High),
        }
    }
}

/// Default per-user history path (honours `XDG_STATE_HOME`).
pub fn default_path() -> PathBuf {
    let base = std::env::var("XDG_STATE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| Path::new(&h).join(".local/state"))
        })
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join("sysmedic/history.jsonl")
}

/// Append one entry, creating parent directories as needed.
pub fn append(path: impl AsRef<Path>, entry: &HistoryEntry) -> Result<(), String> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let line = serde_json::to_string(entry).expect("entry serializes");
    writeln!(file, "{line}").map_err(|e| format!("cannot write history: {e}"))
}

/// Load all recorded entries (skips malformed lines rather than failing).
pub fn load(path: impl AsRef<Path>) -> Vec<HistoryEntry> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A sparkline of the last `max` scores (0–100 mapped over the eight bars).
pub fn sparkline(entries: &[HistoryEntry], max: usize) -> String {
    let start = entries.len().saturating_sub(max);
    entries[start..]
        .iter()
        .map(|e| {
            let idx = ((e.score as usize * (BARS.len() - 1)) / 100).min(BARS.len() - 1);
            BARS[idx]
        })
        .collect()
}

/// The change from the first to the last recorded score, if there are ≥2.
pub fn trend_delta(entries: &[HistoryEntry]) -> Option<i32> {
    match (entries.first(), entries.last()) {
        (Some(a), Some(b)) if entries.len() >= 2 => Some(b.score as i32 - a.score as i32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysmedic_core::{Category, Finding, Snapshot};

    fn report_with(score_findings: &[(Category, Severity)]) -> HealthReport {
        let findings = score_findings
            .iter()
            .map(|(c, s)| Finding::new("t.x", *c, *s, "t", "t"))
            .collect();
        HealthReport::build(Snapshot::default(), findings)
    }

    #[test]
    fn entry_counts_by_severity() {
        let report = report_with(&[
            (Category::Storage, Severity::Critical),
            (Category::Memory, Severity::High),
            (Category::Cpu, Severity::Low),
        ]);
        let entry = HistoryEntry::from_report(&report);
        assert_eq!(entry.findings, 3);
        assert_eq!(entry.critical, 1);
        assert_eq!(entry.high, 1);
    }

    #[test]
    fn append_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sysmedic/history.jsonl");
        let a = HistoryEntry::from_report(&report_with(&[]));
        append(&path, &a).unwrap();
        append(&path, &a).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].score, 100);
    }

    #[test]
    fn missing_file_loads_empty() {
        assert!(load("/nonexistent/history.jsonl").is_empty());
    }

    #[test]
    fn sparkline_maps_scores_to_bars() {
        let mk = |score: u8| HistoryEntry {
            at: "t".into(),
            score,
            grade: "g".into(),
            findings: 0,
            critical: 0,
            high: 0,
        };
        let entries = vec![mk(0), mk(50), mk(100)];
        let spark = sparkline(&entries, 10);
        assert_eq!(spark.chars().count(), 3);
        assert_eq!(spark.chars().next(), Some('▁'));
        assert_eq!(spark.chars().last(), Some('█'));
    }

    #[test]
    fn trend_delta_needs_two_points() {
        let mk = |score: u8| HistoryEntry {
            at: "t".into(),
            score,
            grade: "g".into(),
            findings: 0,
            critical: 0,
            high: 0,
        };
        assert_eq!(trend_delta(&[mk(80)]), None);
        assert_eq!(trend_delta(&[mk(80), mk(92)]), Some(12));
    }
}
