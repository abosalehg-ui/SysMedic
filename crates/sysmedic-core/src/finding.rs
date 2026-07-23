use serde::{Deserialize, Serialize};

/// How serious a finding is. Ordering matters: `Critical > High > ... > Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    /// Points subtracted from the category score for one finding of this severity.
    pub fn penalty(self) -> u32 {
        match self {
            Severity::Info => 0,
            Severity::Low => 5,
            Severity::Medium => 12,
            Severity::High => 25,
            Severity::Critical => 40,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }
}

/// The health category a finding belongs to. Each category is scored
/// independently and contributes to the overall score by its weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Boot,
    Cpu,
    Memory,
    Storage,
    Thermal,
    Processes,
    Services,
    Packages,
    Logs,
    Network,
    Security,
    Battery,
}

impl Category {
    pub const ALL: [Category; 12] = [
        Category::Boot,
        Category::Cpu,
        Category::Memory,
        Category::Storage,
        Category::Thermal,
        Category::Processes,
        Category::Services,
        Category::Packages,
        Category::Logs,
        Category::Network,
        Category::Security,
        Category::Battery,
    ];

    /// Relative weight of the category in the overall health score.
    pub fn weight(self) -> u32 {
        match self {
            Category::Storage | Category::Security => 15,
            Category::Memory | Category::Services => 12,
            Category::Thermal | Category::Packages => 10,
            Category::Boot | Category::Cpu => 8,
            Category::Processes | Category::Logs | Category::Network | Category::Battery => 5,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Boot => "Boot",
            Category::Cpu => "CPU",
            Category::Memory => "Memory",
            Category::Storage => "Storage",
            Category::Thermal => "Thermal",
            Category::Processes => "Processes",
            Category::Services => "Services",
            Category::Packages => "Packages",
            Category::Logs => "Logs",
            Category::Network => "Network",
            Category::Security => "Security",
            Category::Battery => "Battery",
        }
    }
}

/// One diagnosed problem (or notable observation) on the system.
///
/// `id` is a stable machine identifier (e.g. `storage.disk_nearly_full`)
/// used to look up the human explanation in the knowledge base and,
/// from M3 on, the matching fix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub title: String,
    pub category: Category,
    pub severity: Severity,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_hint: Option<String>,
}

impl Finding {
    pub fn new(
        id: &str,
        category: Category,
        severity: Severity,
        title: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Finding {
            id: id.to_string(),
            title: title.into(),
            category,
            severity,
            summary: summary.into(),
            evidence: Vec::new(),
            fix_hint: None,
        }
    }

    pub fn with_evidence(mut self, evidence: Vec<String>) -> Self {
        self.evidence = evidence;
        self
    }

    pub fn with_fix_hint(mut self, hint: impl Into<String>) -> Self {
        self.fix_hint = Some(hint.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_orders_by_seriousness() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Low > Severity::Info);
    }

    #[test]
    fn weights_cover_all_categories() {
        for c in Category::ALL {
            assert!(c.weight() > 0);
        }
    }
}
