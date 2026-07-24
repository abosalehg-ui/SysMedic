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
}

/// The result of a full checkup: an overall 0–100 score, per-category
/// scores, and the findings (already sorted most severe first).
#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub generated_at: String,
    pub score: u8,
    pub grade: &'static str,
    pub category_scores: Vec<CategoryScore>,
    pub findings: Vec<Finding>,
    pub snapshot: Snapshot,
}

impl HealthReport {
    pub fn build(snapshot: Snapshot, findings: Vec<Finding>) -> Self {
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
                }
            })
            .collect();

        let total_weight: u32 = Category::ALL.iter().map(|c| c.weight()).sum();
        let weighted: u32 = category_scores
            .iter()
            .map(|cs| cs.score as u32 * cs.category.weight())
            .sum();
        let raw = (weighted as f64 / total_weight as f64).round() as u8;

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
            category_scores,
            findings,
            snapshot,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::Severity;

    fn finding(category: Category, severity: Severity) -> Finding {
        Finding::new("t.x", category, severity, "t", "t")
    }

    #[test]
    fn healthy_system_scores_100() {
        let report = HealthReport::build(Snapshot::default(), vec![]);
        assert_eq!(report.score, 100);
        assert_eq!(report.grade, "Excellent");
    }

    #[test]
    fn critical_storage_finding_lowers_score() {
        let report = HealthReport::build(
            Snapshot::default(),
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
            Snapshot::default(),
            vec![finding(Category::Storage, Severity::Critical)],
        );
        assert!(report.score <= 59, "score {} was not capped", report.score);
        assert_eq!(report.grade, grade_for(report.score));
        assert_ne!(report.grade, "Excellent");
    }

    #[test]
    fn high_finding_caps_grade_at_fair() {
        let report = HealthReport::build(
            Snapshot::default(),
            vec![finding(Category::Cpu, Severity::High)],
        );
        assert!(report.score <= 74, "score {} was not capped", report.score);
    }

    #[test]
    fn low_findings_do_not_cap() {
        // A couple of Low findings should still leave a healthy overall grade.
        let report = HealthReport::build(
            Snapshot::default(),
            vec![finding(Category::Logs, Severity::Low)],
        );
        assert!(report.score >= 90);
    }

    #[test]
    fn category_score_never_underflows() {
        let findings = (0..5)
            .map(|_| finding(Category::Memory, Severity::Critical))
            .collect();
        let report = HealthReport::build(Snapshot::default(), findings);
        let memory = report
            .category_scores
            .iter()
            .find(|cs| cs.category == Category::Memory)
            .unwrap();
        assert_eq!(memory.score, 0);
    }
}
