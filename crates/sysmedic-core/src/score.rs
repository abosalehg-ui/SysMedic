use std::time::SystemTime;

use serde::Serialize;

use crate::finding::{Category, Finding, Severity};
use crate::snapshot::Snapshot;

/// Highest overall score allowed when any Critical finding is present (top of
/// the "Poor" band).
const CRITICAL_SCORE_CAP: u8 = 59;
/// Highest overall score allowed when the worst finding is High (top of "Fair").
const HIGH_SCORE_CAP: u8 = 74;

#[derive(Debug, Clone, Serialize)]
pub struct CategoryScore {
    pub category: Category,
    pub score: u8,
    /// Whether the checkup actually gathered data for this category. An
    /// unmeasured category scores 100 for want of findings, so display layers
    /// must show it as "not measured" rather than as a clean bill of health.
    pub measured: bool,
}

/// How much of the system the checkup could actually see.
///
/// Surfaced next to the score because the two are only meaningful together:
/// 100/100 over 6 of 12 categories is a different statement from 100/100 over
/// all 12, and the old report made them look identical.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Coverage {
    pub measured: usize,
    pub total: usize,
}

impl Coverage {
    /// True when at least one category could not be measured.
    pub fn is_partial(&self) -> bool {
        self.measured < self.total
    }

    /// "9/12", for display next to the score.
    pub fn label(&self) -> String {
        format!("{}/{}", self.measured, self.total)
    }
}

/// The result of a full checkup: an overall 0–100 score, per-category
/// scores, and the findings (already sorted most severe first).
#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub generated_at: String,
    pub score: u8,
    pub grade: &'static str,
    /// How many categories the score is actually based on.
    pub coverage: Coverage,
    pub category_scores: Vec<CategoryScore>,
    pub findings: Vec<Finding>,
    pub snapshot: Snapshot,
}

impl HealthReport {
    pub fn build(snapshot: Snapshot, findings: Vec<Finding>) -> Self {
        let measured = snapshot.measured_categories();
        let category_scores: Vec<CategoryScore> = Category::ALL
            .iter()
            .map(|&category| {
                let penalty: u32 = findings
                    .iter()
                    .filter(|f| f.category == category)
                    .map(|f| f.severity.penalty())
                    .sum();
                CategoryScore {
                    category,
                    score: 100u32.saturating_sub(penalty) as u8,
                    measured: measured.contains(&category),
                }
            })
            .collect();

        // Average over the categories that were actually measured. Including
        // the unmeasured ones handed each of them a free 100 and diluted the
        // real findings — a container with no systemd, no battery and no SMART
        // scored higher than a laptop with one full disk.
        let scored: Vec<&CategoryScore> = category_scores.iter().filter(|cs| cs.measured).collect();
        let total_weight: u32 = scored.iter().map(|cs| cs.category.weight()).sum();
        let weighted: u32 = scored
            .iter()
            .map(|cs| cs.score as u32 * cs.category.weight())
            .sum();
        // Nothing measured at all: there is no evidence of ill health either,
        // so keep 100 — the 0/12 coverage beside it is what carries the
        // meaning, and inventing a low score from an empty snapshot would be
        // its own kind of lie.
        let raw = if total_weight == 0 {
            100
        } else {
            (weighted as f64 / total_weight as f64).round() as u8
        };

        // Cap the overall score by the most severe finding present. Without
        // this, a single Critical (e.g. a SMART-failing disk) is diluted across
        // twelve weighted categories and the machine still grades "Excellent" —
        // dangerously reassuring. A Critical caps the grade at "Poor", a High at
        // "Fair", so the headline can never contradict a serious finding.
        let worst = findings.iter().map(|f| f.severity).max();
        let score = match worst {
            Some(Severity::Critical) => raw.min(CRITICAL_SCORE_CAP),
            Some(Severity::High) => raw.min(HIGH_SCORE_CAP),
            _ => raw,
        };

        HealthReport {
            generated_at: humantime::format_rfc3339_seconds(SystemTime::now()).to_string(),
            score,
            grade: grade_for(score),
            coverage: Coverage {
                measured: measured.len(),
                total: Category::ALL.len(),
            },
            category_scores,
            findings,
            snapshot,
        }
    }

    /// The most severe finding in the report, if any. Drives the CLI exit code
    /// so a scheduled checkup can be wired into monitoring.
    pub fn worst_severity(&self) -> Option<Severity> {
        self.findings.iter().map(|f| f.severity).max()
    }
}

pub fn grade_for(score: u8) -> &'static str {
    match score {
        90..=100 => "Excellent",
        75..=89 => "Good",
        60..=74 => "Fair",
        40..=59 => "Poor",
        _ => "Critical",
    }
}

/// The grade as user-facing text in the requested language. `HealthReport`
/// keeps the English value (it is serialized and effectively an identifier);
/// display layers call this with the score instead of printing `grade` raw.
pub fn grade_label_in(score: u8, lang: crate::lang::Lang) -> &'static str {
    use crate::lang::Lang;
    match lang {
        Lang::En => grade_for(score),
        Lang::Ar => match score {
            90..=100 => "ممتازة",
            75..=89 => "جيدة",
            60..=74 => "مقبولة",
            40..=59 => "ضعيفة",
            _ => "حرجة",
        },
    }
}

/// "Coverage" as a label, in the requested language.
pub fn coverage_label_in(lang: crate::lang::Lang) -> &'static str {
    match lang {
        crate::lang::Lang::En => "Coverage",
        crate::lang::Lang::Ar => "التغطية",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::Severity;
    use crate::snapshot::{CpuInfo, MemoryInfo};

    fn finding(category: Category, severity: Severity) -> Finding {
        Finding::new("t.x", category, severity, "t", "t")
    }

    /// A snapshot where every category has data, so scoring is not affected by
    /// coverage. Values are deliberately healthy — the findings passed to
    /// `build` are what drive the score.
    fn fully_measured() -> Snapshot {
        use crate::snapshot::*;
        Snapshot {
            cpu: Some(CpuInfo {
                model: "test".into(),
                logical_cores: 8,
                load_1: 0.1,
                load_5: 0.1,
                load_15: 0.1,
            }),
            memory: Some(MemoryInfo {
                total_kb: 1000,
                available_kb: 900,
                swap_total_kb: 0,
                swap_free_kb: 0,
            }),
            disks: Some(vec![DiskInfo {
                mount_point: "/".into(),
                fs_type: "ext4".into(),
                total_bytes: 100,
                available_bytes: 90,
            }]),
            thermal: Some(ThermalInfo { sensors: vec![] }),
            processes: Some(ProcessStats {
                total: 1,
                zombies: vec![],
                top_memory: vec![],
            }),
            services: Some(ServiceStats {
                running: 1,
                failed: vec![],
            }),
            packages: Some(PackageInfo::default()),
            boot: Some(BootInfo {
                total_seconds: 5.0,
                slowest_units: vec![],
            }),
            logs: Some(LogInfo::default()),
            network: Some(NetworkInfo {
                has_default_route: true,
                dns_servers: vec!["1.1.1.1".into()],
            }),
            security: Some(SecurityInfo::default()),
            battery: Some(BatteryInfo::default()),
            ..Default::default()
        }
    }

    #[test]
    fn healthy_system_scores_100() {
        let report = HealthReport::build(fully_measured(), vec![]);
        assert_eq!(report.score, 100);
        assert_eq!(report.grade, "Excellent");
        assert!(!report.coverage.is_partial());
    }

    #[test]
    fn unmeasured_categories_are_excluded_from_the_score() {
        // Only CPU and Memory were collected, and CPU has a High finding.
        // Scoring across all twelve categories diluted that to a comfortable
        // number by averaging in ten categories nobody looked at.
        let snapshot = Snapshot {
            cpu: Some(CpuInfo {
                model: "test".into(),
                logical_cores: 4,
                load_1: 9.0,
                load_5: 9.0,
                load_15: 9.0,
            }),
            memory: Some(MemoryInfo {
                total_kb: 1000,
                available_kb: 900,
                swap_total_kb: 0,
                swap_free_kb: 0,
            }),
            ..Default::default()
        };
        let report = HealthReport::build(snapshot, vec![finding(Category::Cpu, Severity::High)]);
        assert_eq!(report.coverage.measured, 2);
        assert_eq!(report.coverage.total, 12);
        assert!(report.coverage.is_partial());
        assert_eq!(report.coverage.label(), "2/12");
        // Cpu 75 (weight 8) and Memory 100 (weight 12) → 90, then the High cap.
        assert_eq!(report.score, 74);
        // The untouched categories are flagged, not silently perfect.
        let battery = report
            .category_scores
            .iter()
            .find(|cs| cs.category == Category::Battery)
            .unwrap();
        assert!(!battery.measured);
    }

    #[test]
    fn an_empty_snapshot_reports_zero_coverage() {
        let report = HealthReport::build(Snapshot::default(), vec![]);
        assert_eq!(report.coverage.measured, 0);
        assert!(report.coverage.is_partial());
        // No evidence either way: the score stays 100 and the coverage beside
        // it is what tells the user nothing was actually examined.
        assert_eq!(report.score, 100);
    }

    #[test]
    fn critical_storage_finding_lowers_score() {
        let report = HealthReport::build(
            fully_measured(),
            vec![finding(Category::Storage, Severity::Critical)],
        );
        assert!(report.score < 100);
        let storage = report
            .category_scores
            .iter()
            .find(|cs| cs.category == Category::Storage)
            .unwrap();
        assert_eq!(storage.score, 60);
    }

    #[test]
    fn critical_finding_caps_grade_below_excellent() {
        // One Critical, every other category perfect: must not grade Excellent.
        let report = HealthReport::build(
            fully_measured(),
            vec![finding(Category::Storage, Severity::Critical)],
        );
        assert!(report.score <= 59, "score {} was not capped", report.score);
        assert_eq!(report.grade, grade_for(report.score));
        assert_ne!(report.grade, "Excellent");
    }

    #[test]
    fn high_finding_caps_grade_at_fair() {
        let report = HealthReport::build(
            fully_measured(),
            vec![finding(Category::Cpu, Severity::High)],
        );
        assert!(report.score <= 74, "score {} was not capped", report.score);
    }

    #[test]
    fn low_findings_do_not_cap() {
        // A couple of Low findings should still leave a healthy overall grade.
        let report = HealthReport::build(
            fully_measured(),
            vec![finding(Category::Logs, Severity::Low)],
        );
        assert!(report.score >= 90);
    }

    #[test]
    fn category_score_never_underflows() {
        let findings = (0..5)
            .map(|_| finding(Category::Memory, Severity::Critical))
            .collect();
        let report = HealthReport::build(fully_measured(), findings);
        let memory = report
            .category_scores
            .iter()
            .find(|cs| cs.category == Category::Memory)
            .unwrap();
        assert_eq!(memory.score, 0);
    }
}
