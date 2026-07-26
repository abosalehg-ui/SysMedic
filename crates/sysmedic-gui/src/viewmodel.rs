//! Pure presentation logic for the GUI — no GTK types, fully unit-tested.

use sysmedic_core::{HealthReport, Severity};
use sysmedic_knowledge::Lang;

/// All user-facing strings, per language. (Full gettext arrives with M6;
/// until then the two launch languages are built in, mirroring the
/// knowledge base.)
pub struct Strings {
    pub health_score: &'static str,
    pub run_checkup: &'static str,
    pub checking: &'static str,
    pub categories: &'static str,
    pub findings: &'static str,
    pub no_findings: &'static str,
    pub evidence: &'static str,
    pub suggested_command: &'static str,
    pub skipped_checks: &'static str,
    pub checkup_failed: &'static str,
    pub apply_fix: &'static str,
    pub confirm_fix_title: &'static str,
    pub cancel: &'static str,
    pub apply: &'static str,
    pub ok: &'static str,
    pub fix_failed_title: &'static str,
    pub fix_failed_body: &'static str,
    pub reversible_yes: &'static str,
    pub reversible_no: &'static str,
    pub about: &'static str,
    pub app_comment: &'static str,
    pub overview: &'static str,
    pub disk_usage: &'static str,
    pub disk_scanning: &'static str,
    pub disk_scan_failed: &'static str,
    pub disk_empty: &'static str,
    pub disk_largest: &'static str,
    pub history_tooltip: &'static str,
    pub fix_running: &'static str,
    pub fix_applied: &'static str,
    pub export_report: &'static str,
    pub export_first: &'static str,
    pub export_done: &'static str,
    pub export_failed: &'static str,
    pub treemap_a11y: &'static str,
}

impl Strings {
    pub fn for_lang(lang: Lang) -> &'static Strings {
        match lang {
            Lang::En => &Strings {
                health_score: "Health score",
                run_checkup: "Run checkup",
                checking: "Examining your system…",
                categories: "Categories",
                findings: "Findings",
                no_findings: "No problems found — the system looks healthy.",
                evidence: "Evidence",
                suggested_command: "Suggested command",
                skipped_checks: "Skipped checks",
                checkup_failed: "The checkup failed unexpectedly. Please try again.",
                apply_fix: "Apply fix",
                confirm_fix_title: "Apply this fix?",
                cancel: "Cancel",
                apply: "Apply",
                ok: "OK",
                fix_failed_title: "The fix was not applied",
                fix_failed_body:
                    "Authorization was cancelled or failed, or the fix helper is not installed.",
                reversible_yes: "This fix can be undone.",
                reversible_no: "This fix cannot be undone.",
                about: "About SysMedic",
                app_comment:
                    "A doctor for your Linux system: checkup, diagnose, explain, prescribe.",
                overview: "Overview",
                disk_usage: "Disk Usage",
                disk_scanning: "Scanning your home folder…",
                disk_scan_failed: "The disk scan failed. The folder may be unreadable.",
                disk_empty: "Nothing to show — the folder appears to be empty.",
                disk_largest: "Largest entries",
                history_tooltip: "Health-score history (most recent on the right)",
                fix_running: "Applying the fix…",
                fix_applied: "Fix applied — re-checking…",
                export_report: "Export report…",
                export_first: "Run a checkup first.",
                export_done: "Report exported",
                export_failed: "Could not write the report",
                treemap_a11y: "Treemap of the largest folders; the list below has the same data",
            },
            Lang::Ar => &Strings {
                health_score: "الدرجة الصحية",
                run_checkup: "تشغيل الفحص",
                checking: "جارٍ فحص نظامك…",
                categories: "الفئات",
                findings: "المشاكل المكتشفة",
                no_findings: "لا توجد مشاكل — النظام يبدو سليمًا.",
                evidence: "الأدلة",
                suggested_command: "الأمر المقترح",
                skipped_checks: "فحوص متخطاة",
                checkup_failed: "فشل الفحص بشكل غير متوقع. حاول مرة أخرى.",
                apply_fix: "تطبيق الإصلاح",
                confirm_fix_title: "تطبيق هذا الإصلاح؟",
                cancel: "إلغاء",
                apply: "تطبيق",
                ok: "حسنًا",
                fix_failed_title: "لم يُطبَّق الإصلاح",
                fix_failed_body: "أُلغيت المصادقة أو فشلت، أو أن أداة الإصلاح غير مُثبَّتة.",
                reversible_yes: "يمكن التراجع عن هذا الإصلاح.",
                reversible_no: "لا يمكن التراجع عن هذا الإصلاح.",
                about: "عن SysMedic",
                app_comment: "طبيب لنظام لينكس: فحص، تشخيص، شرح، ووصف علاج آمن.",
                overview: "النظرة العامة",
                disk_usage: "استخدام القرص",
                disk_scanning: "جارٍ فحص مجلد المنزل…",
                disk_scan_failed: "فشل فحص القرص. قد يكون المجلد غير قابل للقراءة.",
                disk_empty: "لا شيء لعرضه — يبدو المجلد فارغاً.",
                disk_largest: "أكبر العناصر",
                history_tooltip: "سجل الدرجة الصحية (الأحدث على اليمين)",
                fix_running: "جارٍ تطبيق الإصلاح…",
                fix_applied: "طُبّق الإصلاح — إعادة فحص…",
                export_report: "تصدير التقرير…",
                export_first: "شغّل فحصاً أولاً.",
                export_done: "صُدِّر التقرير",
                export_failed: "تعذّرت كتابة التقرير",
                treemap_a11y: "خريطة شجرية لأكبر المجلدات؛ القائمة أدناه تعرض البيانات نفسها",
            },
        }
    }
}

/// CSS color class for a severity badge (Adwaita style classes).
pub fn severity_css(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "dim-label",
    }
}

/// CSS color class for the big score number.
pub fn score_css(score: u8) -> &'static str {
    match score {
        75..=100 => "success",
        50..=74 => "warning",
        _ => "error",
    }
}

pub struct CategoryRow {
    pub label: &'static str,
    pub score: u8,
}

pub fn category_rows(report: &HealthReport, lang: Lang) -> Vec<CategoryRow> {
    report
        .category_scores
        .iter()
        .map(|cs| CategoryRow {
            label: cs.category.label_in(lang),
            score: cs.score,
        })
        .collect()
}

/// The five explanation lines for a finding, as (question, answer) pairs in
/// the requested language. Empty when the id is unknown.
pub fn explanation_lines(finding_id: &str, lang: Lang) -> Vec<(&'static str, String)> {
    let Some(exp) = sysmedic_knowledge::explain(finding_id, lang) else {
        return Vec::new();
    };
    let questions: [&'static str; 5] = match lang {
        Lang::En => [
            "What caused it?",
            "Is it dangerous?",
            "What is the impact?",
            "How do I fix it?",
            "What if I ignore it?",
        ],
        Lang::Ar => [
            "ما سببها؟",
            "هل هي خطيرة؟",
            "ما تأثيرها؟",
            "كيف أصلحها؟",
            "ماذا لو تجاهلتها؟",
        ],
    };
    vec![
        (questions[0], exp.cause.clone()),
        (questions[1], exp.dangerous.clone()),
        (questions[2], exp.impact.clone()),
        (questions[3], exp.remedy.clone()),
        (questions[4], exp.risk_if_ignored.clone()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysmedic_core::Snapshot;

    #[test]
    fn score_color_thresholds() {
        assert_eq!(score_css(100), "success");
        assert_eq!(score_css(74), "warning");
        assert_eq!(score_css(10), "error");
    }

    #[test]
    fn severity_maps_to_adwaita_classes() {
        assert_eq!(severity_css(Severity::Critical), "error");
        assert_eq!(severity_css(Severity::Medium), "warning");
        assert_eq!(severity_css(Severity::Info), "dim-label");
    }

    #[test]
    fn category_rows_cover_all_categories() {
        let report = HealthReport::build(Snapshot::default(), vec![]);
        assert_eq!(
            category_rows(&report, Lang::En).len(),
            report.category_scores.len()
        );
        // Arabic rows carry Arabic labels, not the English fallback.
        let ar = category_rows(&report, Lang::Ar);
        assert!(ar.iter().any(|r| r.label == "التخزين"));
    }

    #[test]
    fn explanations_exist_in_both_languages() {
        for lang in [Lang::En, Lang::Ar] {
            let lines = explanation_lines("storage.disk_nearly_full", lang);
            assert_eq!(lines.len(), 5);
            assert!(lines.iter().all(|(_, a)| !a.is_empty()));
        }
        assert!(explanation_lines("bogus.id", Lang::En).is_empty());
    }
}
