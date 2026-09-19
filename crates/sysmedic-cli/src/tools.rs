//! `sysmedic disk`, `network`, `monitor` and `history` — the advanced tools.

use std::path::Path;
use std::process::Command;

use anyhow::Result;
use owo_colors::{OwoColorize, Stream};
use sysmedic_core::alert::Alert;
use sysmedic_core::finding::Severity;
use sysmedic_core::score::grade_label_in;
use sysmedic_core::{HealthReport, Lang, Snapshot};

use crate::text::tools;
use sysmedic_diskscan::human_size as human;
use sysmedic_history::HistoryEntry;

fn collect() -> Snapshot {
    sysmedic_collectors::default_snapshot()
}

/// `sysmedic disk [path]`: scan a directory and show the largest subtrees.
pub fn disk(path: Option<String>, depth: u32, top: usize, lang: Lang) -> Result<()> {
    let t = tools(lang);
    let root = path.unwrap_or_else(|| ".".to_string());
    eprintln!("{} {root}…", t.scanning);
    let tree = sysmedic_diskscan::scan(&root, depth.max(1));
    println!(
        "\n  {}  {}\n",
        human(tree.size).if_supports_color(Stream::Stdout, |t| t.bold()),
        Path::new(&root).display()
    );
    let children = sysmedic_diskscan::largest_children(&tree, top);
    if children.is_empty() {
        println!("  {}", t.empty_or_unreadable);
        return Ok(());
    }
    let max = children.first().map(|c| c.size).unwrap_or(1).max(1);
    for child in children {
        let filled = ((child.size as f64 / max as f64) * 20.0).round() as usize;
        let bar: String = "█".repeat(filled) + &"░".repeat(20 - filled);
        // Filenames are attacker-influenceable (a shared directory, an
        // unpacked archive): strip control characters so a crafted name
        // cannot inject terminal escape sequences into our output.
        let name = crate::text::sanitize(&if child.is_dir {
            format!("{}/", child.name)
        } else {
            child.name.clone()
        });
        println!("  {bar}  {:>10}  {name}", human(child.size));
    }
    Ok(())
}

/// `sysmedic network`: default route, DNS, listening ports and latency.
pub fn network(lang: Lang) -> Result<()> {
    let t = tools(lang);
    let snapshot = collect();

    println!(
        "{}",
        t.network.if_supports_color(Stream::Stdout, |t| t.bold())
    );
    match &snapshot.network {
        Some(net) => {
            let route = if net.has_default_route {
                t.yes
                    .if_supports_color(Stream::Stdout, |t| t.green())
                    .to_string()
            } else {
                t.no.if_supports_color(Stream::Stdout, |t| t.red())
                    .to_string()
            };
            println!("  {:<16} {route}", t.default_route);
            let dns = if net.dns_servers.is_empty() {
                t.none
                    .if_supports_color(Stream::Stdout, |t| t.red())
                    .to_string()
            } else {
                net.dns_servers.join(", ")
            };
            println!("  {:<16} {dns}", t.dns_servers);
        }
        None => println!(
            "  {}",
            t.network_unavailable
                .if_supports_color(Stream::Stdout, |t| t.dimmed())
        ),
    }

    match latency("1.1.1.1") {
        Some(ms) => println!("  {:<16} {ms:.1} ms (1.1.1.1)", t.latency),
        None => println!(
            "  {:<16} {}",
            t.latency,
            t.latency_unavailable
                .if_supports_color(Stream::Stdout, |t| t.dimmed())
        ),
    }

    println!(
        "\n{}",
        t.listening_ports
            .if_supports_color(Stream::Stdout, |t| t.bold())
    );
    match &snapshot.ports {
        Some(ports) if !ports.is_empty() => {
            for p in ports {
                let scope = if p.exposed {
                    t.scope_network
                        .if_supports_color(Stream::Stdout, |t| t.yellow())
                        .to_string()
                } else {
                    t.scope_localhost
                        .if_supports_color(Stream::Stdout, |t| t.green())
                        .to_string()
                };
                println!("  {:>5}/{:<5} {:<18} [{scope}]", p.port, p.proto, p.address);
            }
        }
        // Not "TCP ports": UDP has been audited since the sweep was widened.
        _ => println!(
            "  {}",
            t.no_listening_ports
                .if_supports_color(Stream::Stdout, |t| t.dimmed())
        ),
    }
    Ok(())
}

fn full_report() -> HealthReport {
    sysmedic_diagnostics::default_engine().run()
}

/// `sysmedic monitor`: run a checkup, record it in history, and fire a desktop
/// notification for each active alert. This is what the scheduled timer runs.
pub fn monitor(quiet: bool, lang: Lang) -> Result<()> {
    let t = tools(lang);
    let report = full_report();

    // Record history (best-effort — a monitor run should not fail on I/O).
    let entry = HistoryEntry::from_report(&report);
    match sysmedic_history::default_path() {
        Some(path) => {
            if let Err(e) = sysmedic_history::append(path, &entry) {
                eprintln!("warning: could not record history: {e}");
            }
        }
        None => eprintln!("warning: no HOME or XDG_STATE_HOME — history not recorded"),
    }

    let alerts = sysmedic_core::alert::evaluate_in(&report.snapshot, lang);
    for alert in &alerts {
        notify(alert);
    }

    if !quiet {
        println!(
            "{} {}/100 ({}). {} {}",
            t.health_score,
            report.score,
            grade_label_in(report.score, lang),
            alerts.len(),
            t.alerts
        );
        for a in &alerts {
            println!(
                "  {} {}: {}",
                "!".if_supports_color(Stream::Stdout, |t| t.yellow()),
                a.title.if_supports_color(Stream::Stdout, |t| t.bold()),
                a.body
            );
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
            // `--` so a title that ever starts with `-` cannot become an option.
            "--",
            &alert.title,
            &alert.body,
        ])
        .status();
}

/// `sysmedic history`: show the recorded health-score trend.
pub fn history(lang: Lang) -> Result<()> {
    let t = tools(lang);
    let Some(path) = sysmedic_history::default_path() else {
        println!("{}", t.no_history_location);
        return Ok(());
    };
    let entries = sysmedic_history::load(path);
    if entries.is_empty() {
        println!("{}", t.no_history_yet);
        return Ok(());
    }
    println!(
        "{}",
        t.history_title
            .if_supports_color(Stream::Stdout, |t| t.bold())
    );
    println!(
        "  {}",
        sysmedic_history::sparkline(&entries, 40).if_supports_color(Stream::Stdout, |t| t.cyan())
    );
    if let Some(delta) = sysmedic_history::trend_delta(&entries) {
        let text = format!("{delta:+}");
        let colored = if delta >= 0 {
            text.if_supports_color(Stream::Stdout, |t| t.green())
                .to_string()
        } else {
            text.if_supports_color(Stream::Stdout, |t| t.red())
                .to_string()
        };
        println!("  {} {colored}", t.trend_since_first);
    }
    println!();
    for e in entries.iter().rev().take(10) {
        println!(
            "  {}  {:>3}/100  {:<10} {} {}",
            e.at,
            e.score,
            // The stored grade is the English identifier; display layers
            // localize from the score, as the report and dashboard do.
            grade_label_in(e.score, lang),
            e.findings,
            t.findings_count
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
