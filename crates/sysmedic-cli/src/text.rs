//! Colored terminal rendering of a health report.

use std::fmt::Write as _;

use owo_colors::OwoColorize;
use sysmedic_core::{HealthReport, Severity};
use sysmedic_knowledge::{explain, Lang};

pub fn render(report: &HealthReport, lang: Lang) -> String {
    let mut out = String::new();

    let score_line = format!("  Health score: {}/100  ({})", report.score, report.grade);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{}",
        match report.score {
            75..=100 => score_line.green().bold().to_string(),
            50..=74 => score_line.yellow().bold().to_string(),
            _ => score_line.red().bold().to_string(),
        }
    );
    let _ = writeln!(out);

    for cs in &report.category_scores {
        let filled = (cs.score as usize) / 10;
        let bar: String = "█".repeat(filled) + &"░".repeat(10 - filled);
        let _ = writeln!(out, "  {:<10} {} {:>3}", cs.category.label(), bar, cs.score);
    }
    let _ = writeln!(out);

    if report.findings.is_empty() {
        let _ = writeln!(
            out,
            "  {}",
            "No problems found — the system looks healthy.".green()
        );
    } else {
        let _ = writeln!(
            out,
            "  {} finding(s), most severe first:",
            report.findings.len()
        );
        let _ = writeln!(out);
    }

    for f in &report.findings {
        let badge = format!("[{}]", f.severity.label().to_uppercase());
        let badge = match f.severity {
            Severity::Critical | Severity::High => badge.red().bold().to_string(),
            Severity::Medium => badge.yellow().bold().to_string(),
            _ => badge.dimmed().to_string(),
        };
        let _ = writeln!(out, "  {badge} {}", f.title.bold());
        let _ = writeln!(out, "      {}", f.summary);
        if let Some(exp) = explain(&f.id, lang) {
            let _ = writeln!(out, "      {} {}", "Remedy:".cyan(), exp.remedy);
        }
        for e in f.evidence.iter().take(5) {
            let _ = writeln!(out, "        - {}", e.dimmed());
        }
        if let Some(hint) = &f.fix_hint {
            let _ = writeln!(out, "      {} {}", "Try:".cyan(), hint.italic());
        }
        let _ = writeln!(out);
    }

    if !report.snapshot.collection_errors.is_empty() {
        let _ = writeln!(out, "  {}", "Skipped checks:".dimmed());
        for e in &report.snapshot.collection_errors {
            let _ = writeln!(out, "    - {}", e.dimmed());
        }
    }
    out
}
