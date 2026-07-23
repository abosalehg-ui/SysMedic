//! sysmedic-core — the domain layer of SysMedic.
//!
//! Everything here is platform-agnostic and free of I/O: the data model
//! ([`Snapshot`], [`Finding`], [`HealthReport`]), the extension points
//! ([`Collector`], [`Diagnostic`], [`fix::FixPlan`]) and the [`Engine`] that
//! orchestrates a checkup. Infrastructure crates plug into these traits.

pub mod engine;
pub mod finding;
pub mod fix;
pub mod score;
pub mod snapshot;

pub use engine::{Collector, Diagnostic, Engine};
pub use finding::{Category, Finding, Severity};
pub use score::{CategoryScore, HealthReport};
pub use snapshot::Snapshot;
