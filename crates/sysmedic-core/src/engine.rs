use crate::finding::Finding;
use crate::score::HealthReport;
use crate::snapshot::Snapshot;

/// Gathers one section of the [`Snapshot`]. Implementations must never
/// panic and must degrade gracefully: on failure, record a note in
/// `snapshot.collection_errors` and leave the section `None`.
pub trait Collector: Send + Sync {
    fn name(&self) -> &'static str;
    fn collect(&self, snapshot: &mut Snapshot);
}

/// A pure rule that inspects a [`Snapshot`] and reports zero or more
/// [`Finding`]s. Diagnostics must not perform I/O — that is what makes
/// them unit-testable against fixture snapshots.
pub trait Diagnostic: Send + Sync {
    fn name(&self) -> &'static str;
    fn evaluate(&self, snapshot: &Snapshot) -> Vec<Finding>;
}

/// Orchestrates a checkup: run collectors, feed the snapshot through the
/// diagnostic rules, and score the result.
#[derive(Default)]
pub struct Engine {
    collectors: Vec<Box<dyn Collector>>,
    diagnostics: Vec<Box<dyn Diagnostic>>,
}

impl Engine {
    pub fn new() -> Self {
        Engine::default()
    }

    pub fn with_collectors(mut self, collectors: Vec<Box<dyn Collector>>) -> Self {
        self.collectors.extend(collectors);
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: Vec<Box<dyn Diagnostic>>) -> Self {
        self.diagnostics.extend(diagnostics);
        self
    }

    pub fn diagnostic_names(&self) -> Vec<&'static str> {
        self.diagnostics.iter().map(|d| d.name()).collect()
    }

    pub fn run(&self) -> HealthReport {
        let mut snapshot = Snapshot::default();
        for collector in &self.collectors {
            collector.collect(&mut snapshot);
        }
        self.diagnose(snapshot)
    }

    /// Diagnose an already-collected snapshot (used by tests and, later,
    /// by the daemon which collects on its own schedule).
    pub fn diagnose(&self, snapshot: Snapshot) -> HealthReport {
        let mut findings: Vec<Finding> = Vec::new();
        for diagnostic in &self.diagnostics {
            findings.extend(diagnostic.evaluate(&snapshot));
        }
        findings.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.id.cmp(&b.id)));
        HealthReport::build(snapshot, findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::{Category, Severity};

    struct FakeRule;
    impl Diagnostic for FakeRule {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn evaluate(&self, _: &Snapshot) -> Vec<Finding> {
            vec![Finding::new(
                "test.fake",
                Category::Storage,
                Severity::High,
                "Fake",
                "Fake finding",
            )]
        }
    }

    #[test]
    fn engine_runs_diagnostics_and_scores() {
        let report = Engine::new()
            .with_diagnostics(vec![Box::new(FakeRule)])
            .run();
        assert_eq!(report.findings.len(), 1);
        assert!(report.score < 100);
    }
}
