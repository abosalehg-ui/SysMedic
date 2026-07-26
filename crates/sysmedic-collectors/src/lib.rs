//! Linux collectors for SysMedic.
//!
//! Each module keeps a strict split between I/O (reading procfs/sysfs,
//! invoking system tools) and pure parsing functions, so the parsers are
//! unit-tested against fixture strings without touching the live system.
//! Collectors never fail a checkup: anything unreadable is recorded in
//! `Snapshot::collection_errors` and the section stays `None`.

pub mod battery;
pub mod boot;
pub mod cpu;
pub mod disk;
pub mod flatpak;
pub mod logs;
pub mod memory;
pub mod network;
pub mod packages;
pub mod ports;
pub mod process;
pub mod security;
pub mod services;
pub mod smart;
pub mod snap;
pub mod thermal;
mod util;

use sysmedic_core::{Collector, Engine, Snapshot};

/// Collect a [`Snapshot`] with the default collector set — the one
/// composition of "collectors only" shared by every consumer that doesn't
/// need diagnostics (fix applicability, the privileged helper).
pub fn default_snapshot() -> Snapshot {
    Engine::new()
        .with_collectors(default_collectors())
        .run()
        .snapshot
}

/// The full set of collectors for a standard Ubuntu/Debian desktop.
pub fn default_collectors() -> Vec<Box<dyn Collector>> {
    vec![
        Box::new(cpu::CpuCollector),
        Box::new(memory::MemoryCollector),
        Box::new(disk::DiskCollector),
        Box::new(thermal::ThermalCollector),
        Box::new(process::ProcessCollector),
        Box::new(services::ServiceCollector),
        Box::new(packages::PackageCollector),
        Box::new(boot::BootCollector),
        Box::new(logs::LogCollector),
        Box::new(network::NetworkCollector),
        Box::new(security::SecurityCollector),
        Box::new(battery::BatteryCollector),
        Box::new(snap::SnapCollector),
        Box::new(flatpak::FlatpakCollector),
        Box::new(smart::SmartCollector),
        Box::new(ports::PortsCollector),
    ]
}
