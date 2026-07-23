//! The safe-fix contract (executed from M3 via the privileged
//! `sysmedicd` helper; defined here so diagnostics can already point at
//! fixes by id).
//!
//! SysMedic never executes a fix silently. Every fix must first present
//! a [`FixPlan`] answering the three questions the user is owed:
//! *what will happen*, *can it be undone*, and *which files change*.

use serde::Serialize;

use crate::finding::Severity;
use crate::snapshot::Snapshot;

#[derive(Debug, Clone, Serialize)]
pub struct FixPlan {
    /// Stable id, matches the `fix_hint`/knowledge-base id of a finding.
    pub id: String,
    /// Human description of what will happen.
    pub description: String,
    /// Exact commands that would run, in order.
    pub commands: Vec<String>,
    /// Files and directories that will be created, modified or removed.
    pub affected_paths: Vec<String>,
    /// Whether SysMedic can undo this fix from its transaction journal.
    pub reversible: bool,
    /// How to undo it (command or explanation), when `reversible`.
    pub undo: Option<String>,
    /// Residual risk of applying the fix.
    pub risk: Severity,
}

/// Implemented by every fix. `plan` inspects the snapshot and returns
/// `None` when the fix is not applicable; `dry_run` must be side-effect
/// free; `apply` is only ever called after explicit user confirmation.
pub trait Fixer: Send + Sync {
    fn id(&self) -> &'static str;
    fn plan(&self, snapshot: &Snapshot) -> Option<FixPlan>;
    fn dry_run(&self, plan: &FixPlan) -> Result<String, String>;
    fn apply(&self, plan: &FixPlan) -> Result<String, String>;
}
