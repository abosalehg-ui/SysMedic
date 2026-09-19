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

/// Chrome for the commands beyond `checkup` and `explain`.
///
/// `fix`, `undo`, `network`, `monitor`, `history` and `schedule` printed pure
/// English whatever the locale, so an Arabic user got an Arabic window, an
/// Arabic checkup, and then `sysmedic fix` in English. The knowledge base and
/// the GUI have been bilingual since the beginning; the rest of the terminal
/// had simply never been given a table to read from.
pub struct Tools {
    pub no_fixes: &'static str,
    pub applicable_fixes: &'static str,
    pub reversible: &'static str,
    pub not_reversible: &'static str,
    pub dry_run_note: &'static str,
    pub rerun_to_apply: &'static str,
    pub applied: &'static str,
    pub would_undo: &'static str,
    pub rerun_to_undo: &'static str,
    pub nothing_to_undo: &'static str,
    pub cannot_read_journal: &'static str,
    pub reverted: &'static str,
    pub requesting_auth: &'static str,
    pub scanning: &'static str,
    pub empty_or_unreadable: &'static str,
    pub network: &'static str,
    pub default_route: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
    pub dns_servers: &'static str,
    pub none: &'static str,
    pub latency: &'static str,
    pub latency_unavailable: &'static str,
    pub network_unavailable: &'static str,
    pub listening_ports: &'static str,
    pub no_listening_ports: &'static str,
    pub scope_network: &'static str,
    pub scope_localhost: &'static str,
    pub health_score: &'static str,
    pub alerts: &'static str,
    pub no_history_location: &'static str,
    pub no_history_yet: &'static str,
    pub history_title: &'static str,
    pub trend_since_first: &'static str,
    pub findings_count: &'static str,
    pub schedule_enabled: &'static str,
    pub schedule_not_activated: &'static str,
    pub schedule_disabled: &'static str,
    pub schedule_status_off: &'static str,
    pub schedule_status_on: &'static str,
    pub cadence_daily: &'static str,
    pub cadence_weekly: &'static str,
    pub cadence_monthly: &'static str,
}

/// The chrome for those commands, in `lang`.
pub fn tools(lang: Lang) -> &'static Tools {
    match lang {
        Lang::En => &Tools {
            no_fixes: "No fixes needed — nothing to prescribe.",
            applicable_fixes: "Applicable fixes (run `sysmedic fix <id> --dry-run` to preview):",
            reversible: "reversible",
            not_reversible: "not reversible",
            dry_run_note: "(dry run — nothing was changed)",
            rerun_to_apply: "Re-run with --yes to apply this fix.",
            applied: "applied",
            would_undo: "Would undo:",
            rerun_to_undo: "Re-run with --yes to undo.",
            nothing_to_undo: "Nothing to undo.",
            cannot_read_journal: "(cannot read journal:",
            reverted: "reverted",
            requesting_auth: "Requesting authorization",
            scanning: "Scanning",
            empty_or_unreadable: "(empty or unreadable)",
            network: "Network",
            default_route: "Default route:",
            yes: "yes",
            no: "no",
            dns_servers: "DNS servers:",
            none: "(none)",
            latency: "Latency:",
            latency_unavailable: "unavailable (no ping)",
            network_unavailable: "network info unavailable",
            listening_ports: "Listening ports",
            // Not "TCP ports": the collector has audited UDP since the port
            // sweep was widened, and the message said otherwise.
            no_listening_ports: "no listening ports found",
            scope_network: "network",
            scope_localhost: "localhost",
            health_score: "Health score:",
            alerts: "alert(s).",
            no_history_location:
                "No history location available (neither HOME nor XDG_STATE_HOME is set).",
            no_history_yet:
                "No history yet. Run `sysmedic monitor` or enable `sysmedic schedule daily`.",
            history_title: "Health-score history",
            trend_since_first: "Trend since first record:",
            findings_count: "finding(s)",
            schedule_enabled: "Scheduled checkups. See status with `systemctl --user list-timers`.",
            schedule_not_activated: "timer units installed, but they could not be activated here \
                                     (no user systemd session). On your desktop run:",
            schedule_disabled: "Scheduled checkups disabled.",
            schedule_status_off: "Scheduled checkups:",
            schedule_status_on: "Scheduled checkups:",
            cadence_daily: "daily",
            cadence_weekly: "weekly",
            cadence_monthly: "monthly",
        },
        Lang::Ar => &Tools {
            no_fixes: "لا حاجة لأي إصلاح — لا شيء يُوصف.",
            applicable_fixes: "الإصلاحات المنطبقة (شغّل ‎`sysmedic fix <id> --dry-run`‎ للمعاينة):",
            reversible: "قابل للتراجع",
            not_reversible: "غير قابل للتراجع",
            dry_run_note: "(تشغيل تجريبي — لم يُغيَّر شيء)",
            rerun_to_apply: "أعد التشغيل مع ‎--yes‎ لتطبيق هذا الإصلاح.",
            applied: "طُبّق",
            would_undo: "سيُتراجَع عن:",
            rerun_to_undo: "أعد التشغيل مع ‎--yes‎ للتراجع.",
            nothing_to_undo: "لا يوجد ما يُتراجَع عنه.",
            cannot_read_journal: "(تعذّرت قراءة السجل:",
            reverted: "تُراجِع عن",
            requesting_auth: "جارٍ طلب التفويض",
            scanning: "جارٍ فحص",
            empty_or_unreadable: "(فارغ أو غير قابل للقراءة)",
            network: "الشبكة",
            default_route: "المسار الافتراضي:",
            yes: "نعم",
            no: "لا",
            dns_servers: "خوادم DNS:",
            none: "(لا يوجد)",
            latency: "زمن الاستجابة:",
            latency_unavailable: "غير متاح (لا يوجد ping)",
            network_unavailable: "معلومات الشبكة غير متاحة",
            listening_ports: "المنافذ المُنصتة",
            no_listening_ports: "لم يُعثر على منافذ مُنصتة",
            scope_network: "الشبكة",
            scope_localhost: "محلي",
            health_score: "الدرجة الصحية:",
            alerts: "تنبيه.",
            no_history_location: "لا يوجد مكان لحفظ السجل (لا HOME ولا XDG_STATE_HOME مضبوط).",
            no_history_yet:
                "لا يوجد سجل بعد. شغّل ‎`sysmedic monitor`‎ أو فعّل ‎`sysmedic schedule daily`‎.",
            history_title: "سجل الدرجة الصحية",
            trend_since_first: "التغيّر منذ أول تسجيل:",
            findings_count: "نتيجة",
            schedule_enabled: "جُدولت الفحوص. اعرض الحالة بـ ‎`systemctl --user list-timers`‎.",
            schedule_not_activated: "رُكِّبت وحدات المؤقّت، لكن تعذّر تفعيلها هنا (لا توجد جلسة \
                                     systemd للمستخدم). على جهازك المكتبي نفّذ:",
            schedule_disabled: "أُوقفت الفحوص المجدولة.",
            schedule_status_off: "الفحوص المجدولة:",
            schedule_status_on: "الفحوص المجدولة:",
            cadence_daily: "يومية",
            cadence_weekly: "أسبوعية",
            cadence_monthly: "شهرية",
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
