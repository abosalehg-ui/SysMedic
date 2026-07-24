//! `sysmedic disk`, `network`, `monitor` and `history` — the advanced tools.

use std::path::Path;
use std::process::Command;

use anyhow::Result;
use owo_colors::OwoColorize;
use sysmedic_core::alert::Alert;
use sysmedic_core::finding::Severity;
use sysmedic_core::{HealthReport, Snapshot};
use sysmedic_diskscan::human_size as human;
use sysmedic_history::HistoryEntry;

fn collect() -> Snapshot {
    sysmedic_core::Engine::new()
        .with_collectors(sysmedic_collectors::default_collectors())
        .run()
        .snapshot
}

/// `sysmedic disk [path]`: scan a directory and show the largest subtrees.
pub fn disk(path: Option<String>, depth: u32, top: usize) -> Result<()> {
    let root = path.unwrap_or_else(|| ".".to_string());
    eprintln!("Scanning {root}…");
    let tree = sysmedic_diskscan::scan(&root, depth.max(1));
    println!(
        "\n  {}  {}\n",
        human(tree.size).bold(),
        Path::new(&root).display()
    );
    let children = sysmedic_diskscan::largest_children(&tree, top);
    if children.is_empty() {
        println!("  (empty or unreadable)");
        return Ok(());
    }
    let max = children.first().map(|c| c.size).unwrap_or(1).max(1);
    for child in children {
        let filled = ((child.size as f64 / max as f64) * 20.0).round() as usize;
        let bar: String = "█".repeat(filled) + &"░".repeat(20 - filled);
        let name = if child.is_dir {
            format!("{}/", child.name)
        } else {
            child.name.clone()
        };
        println!("  {bar}  {:>10}  {name}", human(child.size));
    }
    Ok(())
}

/// `sysmedic network`: default route, DNS, listening ports and latency.
pub fn network() -> Result<()> {
    let snapshot = collect();

    println!("{}", "Network".bold());
    match &snapshot.network {
        Some(net) => {
            let route = if net.has_default_route {
                "yes".green().to_string()
            } else {
                "no".red().to_string()
            };
            println!("  Default route:  {route}");
            let dns = if net.dns_servers.is_empty() {
                "(none)".red().to_string()
            } else {
                net.dns_servers.join(", ")
            };
            println!("  DNS servers:    {dns}");
        }
        None => println!("  {}", "network info unavailable".dimmed()),
    }

    match latency("1.1.1.1") {
        Some(ms) => println!("  Latency:        {ms:.1} ms (1.1.1.1)"),
        None => println!("  Latency:        {}", "unavailable (no ping)".dimmed()),
    }

    println!("\n{}", "Listening ports".bold());
    match &snapshot.ports {
        Some(ports) if !ports.is_empty() => {
            for p in ports {
                let scope = if p.exposed {
                    "network".yellow().to_string()
                } else {
                    "localhost".green().to_string()
                };
                println!("  {:>5}/{:<5} {:<18} [{scope}]", p.port, p.proto, p.address);
            }
        }
        _ => println!("  {}", "no listening TCP ports found".dimmed()),
    }
    Ok(())
}

fn full_report() -> HealthReport {
    sysmedic_core::Engine::new()
        .with_collectors(sysmedic_collectors::default_collectors())
        .with_diagnostics(sysmedic_diagnostics::default_diagnostics())
        .run()
}

/// `sysmedic monitor`: run a checkup, record it in history, and fire a desktop
/// notification for each active alert. This is what the scheduled timer runs.
pub fn monitor(quiet: bool) -> Result<()> {
    let report = full_report();

    // Record history (best-effort — a monitor run should not fail on I/O).
    let entry = HistoryEntry::from_report(&report);
    if let Err(e) = sysmedic_history::append(sysmedic_history::default_path(), &entry) {
        eprintln!("warning: could not record history: {e}");
    }

    let alerts = sysmedic_core::alert::evaluate(&report.snapshot);
    for alert in &alerts {
        notify(alert);
    }

    if !quiet {
        println!(
            "Health score: {}/100 ({}). {} alert(s).",
            report.score,
            report.grade,
            alerts.len()
        );
        for a in &alerts {
            println!("  {} {}: {}", "!".yellow(), a.title.bold(), a.body);
        }
    }
    Ok(())
}

/// Send a desktop notification via `notify-send` (no-op if unavailable).
fn notify(alert: &Alert) {
    let urgency = match alert.urgency {
        Severity::Critical | Severity::High => "critical",
        Severity::Medium => "normal",
        _ => "low",
    };
    let _ = Command::new("notify-send")
        .args([
            "--app-name=SysMedic",
            &format!("--urgency={urgency}"),
            &alert.title,
            &alert.body,
        ])
        .status();
}

/// `sysmedic history`: show the recorded health-score trend.
pub fn history() -> Result<()> {
    let entries = sysmedic_history::load(sysmedic_history::default_path());
    if entries.is_empty() {
        println!("No history yet. Run `sysmedic monitor` or enable `sysmedic schedule daily`.");
        return Ok(());
    }
    println!("{}", "Health-score history".bold());
    println!("  {}", sysmedic_history::sparkline(&entries, 40).cyan());
    if let Some(delta) = sysmedic_history::trend_delta(&entries) {
        let text = format!("{delta:+}");
        let colored = if delta >= 0 {
            text.green().to_string()
        } else {
            text.red().to_string()
        };
        println!("  Trend since first record: {colored}");
    }
    println!();
    for e in entries.iter().rev().take(10) {
        println!(
            "  {}  {:>3}/100  {:<10} {} finding(s)",
            e.at, e.score, e.grade, e.findings
        );
    }
    Ok(())
}

/// Round-trip latency to `host` in milliseconds, via `ping -c1 -w1`.
fn latency(host: &str) -> Option<f64> {
    let out = Command::new("ping")
        .args(["-c", "1", "-w", "1", host])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_ping_rtt(&String::from_utf8_lossy(&out.stdout))
}

/// Extract the RTT from a `ping` line like `time=12.3 ms`.
fn parse_ping_rtt(output: &str) -> Option<f64> {
    let idx = output.find("time=")?;
    output[idx + 5..]
        .split_whitespace()
        .next()?
        .parse::<f64>()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_sizes() {
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1536), "1.5 KiB");
        assert_eq!(human(5 * 1024 * 1024 * 1024), "5.0 GiB");
    }

    #[test]
    fn parses_ping_rtt() {
        let sample = "64 bytes from 1.1.1.1: icmp_seq=1 ttl=57 time=12.3 ms\n";
        assert_eq!(parse_ping_rtt(sample), Some(12.3));
        assert_eq!(parse_ping_rtt("no timing here"), None);
    }
}
