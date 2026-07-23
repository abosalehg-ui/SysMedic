//! `sysmedic disk` and `sysmedic network` — the M4 advanced tools on the CLI.

use std::path::Path;
use std::process::Command;

use anyhow::Result;
use owo_colors::OwoColorize;
use sysmedic_core::Snapshot;

fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

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
