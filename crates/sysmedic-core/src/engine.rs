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
        // Collectors are independent (each fills only its own snapshot
        // sections) and dominated by subprocess wait time — `apt list` alone
        // takes seconds, and a hung tool burns its full 10 s timeout. Running
        // them serially made the checkup as slow as the *sum* of its tools;
        // scoped threads make it as slow as the slowest one. Results are
        // merged in declaration order, so output stays deterministic.
        let mut snapshot = Snapshot::default();
        let partials: Vec<Result<Snapshot, &'static str>> = std::thread::scope(|scope| {
            let handles: Vec<_> = self
                .collectors
                .iter()
                .map(|collector| {
                    let name = collector.name();
                    (
                        name,
                        scope.spawn(move || {
                            let mut partial = Snapshot::default();
                            collector.collect(&mut partial);
                            partial
                        }),
                    )
                })
                .collect();
            handles
                .into_iter()
                .map(|(name, handle)| handle.join().map_err(|_| name))
                .collect()
        });
        for partial in partials {
            match partial {
                Ok(p) => snapshot.merge(p),
                // A panicking collector violates its contract; degrade to a
                // skipped check instead of poisoning the whole checkup.
                Err(name) => snapshot
                    .collection_errors
                    .push(format!("{name}: collector panicked")),
            }
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

    struct MemoryCollector;
    impl Collector for MemoryCollector {
        fn name(&self) -> &'static str {
            "memory"
        }
        fn collect(&self, snapshot: &mut Snapshot) {
            snapshot.memory = Some(crate::snapshot::MemoryInfo {
                total_kb: 1000,
                available_kb: 500,
                swap_total_kb: 0,
                swap_free_kb: 0,
            });
        }
    }

    struct FailingCollector;
    impl Collector for FailingCollector {
        fn name(&self) -> &'static str {
            "failing"
        }
        fn collect(&self, snapshot: &mut Snapshot) {
            snapshot.collection_errors.push("failing: no tool".into());
        }
    }

    struct PanickingCollector;
    impl Collector for PanickingCollector {
        fn name(&self) -> &'static str {
            "panicking"
        }
        fn collect(&self, _: &mut Snapshot) {
            panic!("contract violation");
        }
    }

    #[test]
    fn parallel_collectors_merge_sections_and_errors() {
        let report = Engine::new()
            .with_collectors(vec![Box::new(MemoryCollector), Box::new(FailingCollector)])
            .run();
        assert!(report.snapshot.memory.is_some());
        assert_eq!(
            report.snapshot.collection_errors,
            vec!["failing: no tool".to_string()]
        );
    }

    #[test]
    fn panicking_collector_degrades_to_skipped_check() {
        let report = Engine::new()
            .with_collectors(vec![
                Box::new(PanickingCollector),
                Box::new(MemoryCollector),
            ])
            .run();
        assert!(report.snapshot.memory.is_some());
        assert!(report
            .snapshot
            .collection_errors
            .iter()
            .any(|e| e.contains("panicking")));
    }
}
