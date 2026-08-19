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
    pub disk_choose_folder: &'static str,
    pub disk_cancel: &'static str,
    pub disk_cancelled: &'static str,
    pub disk_partial: &'static str,
    pub disk_pick_a_folder: &'static str,
    pub coverage_note: &'static str,
    pub not_measured: &'static str,
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
                disk_choose_folder: "Choose a folder to scan…",
                disk_cancel: "Stop the scan",
                disk_cancelled: "Scan stopped — nothing measured yet.",
                disk_partial: "Partial scan · ",
                disk_pick_a_folder: "Choose a folder to scan.",
                coverage_note: "categories measured; the score is computed from those only",
                not_measured: "not measured",
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
                disk_choose_folder: "اختر مجلداً للفحص…",
                disk_cancel: "إيقاف الفحص",
                disk_cancelled: "أُوقف الفحص — لم يُقَس شيء بعد.",
                disk_partial: "فحص جزئي · ",
                disk_pick_a_folder: "اختر مجلداً لفحصه.",
                coverage_note: "فئة مقيسة؛ والدرجة محسوبة منها وحدها",
                not_measured: "غير مقيس",
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
    /// False when the checkup gathered no data for this category, so the view
    /// can say "not measured" instead of drawing a full bar.
    pub measured: bool,
}

pub fn category_rows(report: &HealthReport, lang: Lang) -> Vec<CategoryRow> {
    report
        .category_scores
        .iter()
        .map(|cs| CategoryRow {
            label: cs.category.label_in(lang),
            score: cs.score,
            measured: cs.measured,
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

    /// Every field of [`Strings`], as `(name, value)` pairs.
    ///
    /// Hand-listing a subset meant a newly added string could ship with an
    /// empty or untranslated Arabic value and no test would notice — the old
    /// checks covered nine of the (then) forty-five fields. Adding a field to
    /// the struct without adding it here fails to compile, because the
    /// destructuring below is exhaustive.
    fn all_strings(s: &'static Strings) -> Vec<(&'static str, &'static str)> {
        // Destructured exhaustively on purpose: `..` is deliberately absent.
        let Strings {
            health_score,
            run_checkup,
            checking,
            categories,
            findings,
            no_findings,
            evidence,
            suggested_command,
            skipped_checks,
            checkup_failed,
            apply_fix,
            confirm_fix_title,
            cancel,
            apply,
            ok,
            fix_failed_title,
            fix_failed_body,
            reversible_yes,
            reversible_no,
            about,
            app_comment,
            overview,
            disk_usage,
            disk_scanning,
            disk_scan_failed,
            disk_empty,
            disk_largest,
            history_tooltip,
            fix_running,
            fix_applied,
            export_report,
            export_first,
            export_done,
            export_failed,
            treemap_a11y,
            disk_choose_folder,
            disk_cancel,
            disk_cancelled,
            disk_partial,
            disk_pick_a_folder,
            coverage_note,
            not_measured,
        } = s;
        vec![
            ("health_score", health_score),
            ("run_checkup", run_checkup),
            ("checking", checking),
            ("categories", categories),
            ("findings", findings),
            ("no_findings", no_findings),
            ("evidence", evidence),
            ("suggested_command", suggested_command),
            ("skipped_checks", skipped_checks),
            ("checkup_failed", checkup_failed),
            ("apply_fix", apply_fix),
            ("confirm_fix_title", confirm_fix_title),
            ("cancel", cancel),
            ("apply", apply),
            ("ok", ok),
            ("fix_failed_title", fix_failed_title),
            ("fix_failed_body", fix_failed_body),
            ("reversible_yes", reversible_yes),
            ("reversible_no", reversible_no),
            ("about", about),
            ("app_comment", app_comment),
            ("overview", overview),
            ("disk_usage", disk_usage),
            ("disk_scanning", disk_scanning),
            ("disk_scan_failed", disk_scan_failed),
            ("disk_empty", disk_empty),
            ("disk_largest", disk_largest),
            ("history_tooltip", history_tooltip),
            ("fix_running", fix_running),
            ("fix_applied", fix_applied),
            ("export_report", export_report),
            ("export_first", export_first),
            ("export_done", export_done),
            ("export_failed", export_failed),
            ("treemap_a11y", treemap_a11y),
            ("disk_choose_folder", disk_choose_folder),
            ("disk_cancel", disk_cancel),
            ("disk_cancelled", disk_cancelled),
            ("disk_partial", disk_partial),
            ("disk_pick_a_folder", disk_pick_a_folder),
            ("coverage_note", coverage_note),
            ("not_measured", not_measured),
        ]
    }

    #[test]
    fn no_string_is_empty_in_either_language() {
        // A missing translation shows as a blank label rather than a fallback,
        // so an empty string is a bug in either language.
        for lang in [Lang::En, Lang::Ar] {
            for (name, value) in all_strings(Strings::for_lang(lang)) {
                assert!(!value.trim().is_empty(), "{name} is empty for {lang:?}");
            }
        }
    }

    #[test]
    fn every_arabic_string_is_actually_arabic() {
        // Guards against a copy-paste that leaves an English string in the
        // Arabic table — which the emptiness check above would not catch.
        // `disk_partial` is a prefix ending in a separator, and `app_comment`
        // and the rest still have to carry Arabic script.
        for (name, value) in all_strings(Strings::for_lang(Lang::Ar)) {
            assert!(
                value
                    .chars()
                    .any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)),
                "{name} has no Arabic characters: {value}"
            );
        }
    }

    #[test]
    fn the_two_tables_have_the_same_fields_in_the_same_order() {
        let en = all_strings(Strings::for_lang(Lang::En));
        let ar = all_strings(Strings::for_lang(Lang::Ar));
        assert_eq!(
            en.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            ar.iter().map(|(n, _)| *n).collect::<Vec<_>>()
        );
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
