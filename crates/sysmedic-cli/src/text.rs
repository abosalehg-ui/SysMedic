//! Colored terminal rendering of a health report.

use std::fmt::Write as _;

use owo_colors::OwoColorize;
use sysmedic_core::{HealthReport, Severity};
use sysmedic_knowledge::{explain, Lang};

/// Localized chrome for the terminal report. Finding bodies come from the
/// rules (English for now) and remedies from the bilingual knowledge base;
/// this keeps the surrounding labels in the same language as the remedies.
struct Chrome {
    health_score: &'static str,
    healthy: &'static str,
    findings_intro_prefix: &'static str,
    findings_intro_suffix: &'static str,
    remedy: &'static str,
    try_: &'static str,
    skipped: &'static str,
    coverage: &'static str,
    coverage_note: &'static str,
    not_measured: &'static str,
}

fn chrome(lang: Lang) -> Chrome {
    match lang {
        Lang::En => Chrome {
            health_score: "Health score",
            healthy: "No problems found — the system looks healthy.",
            findings_intro_prefix: "",
            findings_intro_suffix: " finding(s), most severe first:",
            remedy: "Remedy:",
            try_: "Try:",
            skipped: "Skipped checks:",
            coverage: "Coverage",
            coverage_note: "categories measured; the score is computed from those only",
            not_measured: "not measured",
        },
        Lang::Ar => Chrome {
            health_score: "الدرجة الصحية",
            healthy: "لا توجد مشاكل — النظام يبدو سليماً.",
            findings_intro_prefix: "عدد النتائج: ",
            findings_intro_suffix: "، الأخطر أولاً:",
            remedy: "العلاج:",
            try_: "جرّب:",
            skipped: "فحوص متخطاة:",
            coverage: "التغطية",
            coverage_note: "فئة مقيسة؛ والدرجة محسوبة منها وحدها",
            not_measured: "غير مقيس",
        },
    }
}

/// Field labels for `sysmedic explain`, aligned so the answers line up.
pub struct ExplainLabels {
    pub cause: &'static str,
    pub dangerous: &'static str,
    pub impact: &'static str,
    pub remedy: &'static str,
    pub if_ignored: &'static str,
}

/// Labels for the five explanation questions, in `lang`.
///
/// The answers come from the bilingual knowledge base, so printing them under
/// hardcoded English labels left `sysmedic explain <id> --lang ar` half
/// translated.
pub fn explain_labels(lang: Lang) -> ExplainLabels {
    match lang {
        Lang::En => ExplainLabels {
            cause: "Cause:         ",
            dangerous: "Dangerous?     ",
            impact: "Impact:        ",
            remedy: "Remedy:        ",
            if_ignored: "If ignored:    ",
        },
        Lang::Ar => ExplainLabels {
            cause: "السبب:",
            dangerous: "هل هي خطيرة؟",
            impact: "التأثير:",
            remedy: "العلاج:",
            if_ignored: "إذا أُهملت:",
        },
    }
}

pub fn render(report: &HealthReport, lang: Lang) -> String {
    let mut out = String::new();
    let c = chrome(lang);

    let score_line = format!(
        "  {}: {}/100  ({})",
        c.health_score,
        report.score,
        sysmedic_core::score::grade_label_in(report.score, lang)
    );
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

    // State the coverage next to the score: 100/100 across six of twelve
    // categories is a different claim from 100/100 across all of them, and the
    // report used to render the two identically.
    if report.coverage.is_partial() {
        let _ = writeln!(
            out,
            "  {}",
            format!(
                "{}: {} — {}",
                c.coverage,
                report.coverage.label(),
                c.coverage_note
            )
            .dimmed()
        );
        let _ = writeln!(out);
    }

    for cs in &report.category_scores {
        if !cs.measured {
            let _ = writeln!(
                out,
                "  {:<10} {}",
                cs.category.label_in(lang),
                c.not_measured.dimmed()
            );
            continue;
        }
        let filled = (cs.score as usize) / 10;
        let bar: String = "█".repeat(filled) + &"░".repeat(10 - filled);
        let _ = writeln!(
            out,
            "  {:<10} {} {:>3}",
            cs.category.label_in(lang),
            bar,
            cs.score
        );
    }
    let _ = writeln!(out);

    if report.findings.is_empty() {
        let _ = writeln!(out, "  {}", c.healthy.green());
    } else {
        let _ = writeln!(
            out,
            "  {}{}{}",
            c.findings_intro_prefix,
            report.findings.len(),
            c.findings_intro_suffix
        );
        let _ = writeln!(out);
    }

    for f in &report.findings {
        // The user-facing label, not the machine one: an Arabic report used to
        // print `[CRITICAL]` above an Arabic sentence. `Severity::label()`
        // stays for CSS classes and JSON.
        let badge = format!("[{}]", f.severity.label_in(lang).to_uppercase());
        let badge = match f.severity {
            Severity::Critical | Severity::High => badge.red().bold().to_string(),
            Severity::Medium => badge.yellow().bold().to_string(),
            _ => badge.dimmed().to_string(),
        };
        let title = sysmedic_knowledge::localized_title(f, lang);
        let summary = sysmedic_knowledge::localized_summary(f, lang);
        let _ = writeln!(out, "  {badge} {}", sanitize(&title).bold());
        let _ = writeln!(out, "      {}", sanitize(&summary));
        let _ = writeln!(out, "      {}", format!("id: {}", f.id).dimmed());
        if let Some(exp) = explain(&f.id, lang) {
            let _ = writeln!(out, "      {} {}", c.remedy.cyan(), exp.remedy);
        }
        for e in f.evidence.iter().take(5) {
            let _ = writeln!(out, "        - {}", sanitize(e).dimmed());
        }
        if let Some(hint) = &f.fix_hint {
            let _ = writeln!(out, "      {} {}", c.try_.cyan(), sanitize(hint).italic());
        }
        let _ = writeln!(out);
    }

    if !report.snapshot.collection_errors.is_empty() {
        let _ = writeln!(out, "  {}", c.skipped.dimmed());
        for e in &report.snapshot.collection_errors {
            let _ = writeln!(out, "    - {}", e.dimmed());
        }
    }
    out
}

/// Strip C0/C1 control characters (keeping `\n` and `\t`) from a string that
/// embeds system data — process names from `/proc/*/comm`, mount labels, LLM
/// output. A local process named with embedded `\x1b[…` bytes must not be able
/// to inject terminal control sequences into our output.
pub fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// Remove ANSI SGR escape sequences. Used when the rendered report is written
/// to a file or a pipe (or `NO_COLOR` is set), so it isn't polluted with raw
/// `\x1b[..m` codes.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Consume a CSI sequence: '[' then params up to a letter terminator.
            if let Some('[') = chars.clone().next() {
                chars.next();
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_color_codes() {
        let colored = format!("{}{}{}", "\u{1b}[32m", "hi", "\u{1b}[0m");
        assert_eq!(strip_ansi(&colored), "hi");
    }

    #[test]
    fn leaves_plain_text_untouched() {
        assert_eq!(strip_ansi("no color here"), "no color here");
    }

    #[test]
    fn sanitize_strips_control_chars_but_keeps_whitespace() {
        assert_eq!(sanitize("evil\u{1b}[2Jname"), "evil[2Jname");
        assert_eq!(sanitize("line\nnext\ttab"), "line\nnext\ttab");
        assert_eq!(sanitize("bell\u{7}cr\u{d}"), "bellcr");
    }
}
