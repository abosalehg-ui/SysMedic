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
    pub trend_since_first: &'static str,
    pub undo_last_fix: &'static str,
    pub undo_action: &'static str,
    pub undo_confirm_title: &'static str,
    pub undo_nothing: &'static str,
    pub undo_running: &'static str,
    pub undo_done: &'static str,
    pub undo_failed_title: &'static str,
    pub fix_cancelled_body: &'static str,
    pub fix_unauthorized_body: &'static str,
    pub disk_scanning_path: &'static str,
    pub disk_entries: &'static str,
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
                trend_since_first: "since first record",
                undo_last_fix: "Undo last fix…",
                undo_action: "Undo",
                undo_confirm_title: "Undo this fix?",
                undo_nothing: "Nothing to undo.",
                undo_running: "Undoing…",
                undo_done: "Fix undone — re-checking…",
                undo_failed_title: "The fix was not undone",
                fix_cancelled_body: "Authorization was cancelled, so nothing was changed.",
                fix_unauthorized_body:
                    "Authorization was refused, or the fix helper is not installed. \
                     It ships with the .deb package.",
                disk_scanning_path: "Scanning",
                disk_entries: "entries",
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
                trend_since_first: "منذ أول تسجيل",
                undo_last_fix: "التراجع عن آخر إصلاح…",
                undo_action: "تراجع",
                undo_confirm_title: "التراجع عن هذا الإصلاح؟",
                undo_nothing: "لا يوجد ما يُتراجَع عنه.",
                undo_running: "جارٍ التراجع…",
                undo_done: "تُراجِع عن الإصلاح — إعادة فحص…",
                undo_failed_title: "لم يتم التراجع",
                fix_cancelled_body: "أُلغيت المصادقة، فلم يتغيّر شيء.",
                fix_unauthorized_body:
                    "رُفضت المصادقة، أو أن أداة الإصلاح غير مُثبَّتة. تأتي مع حزمة ‎.deb‎.",
                disk_scanning_path: "جارٍ فحص",
                disk_entries: "عنصر",
            },
        }
    }
}

/// Why a privileged operation did not happen.
///
/// The dialog used to say one thing — "Authorization was cancelled or failed,
/// or the fix helper is not installed" — for three different situations, and
/// the helper's own message was thrown away with it. Someone who dismissed the
/// password prompt themselves read the same words as someone whose `apt-get`
/// died on a held dpkg lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperFailure {
    /// pkexec exit 126: the user dismissed the authentication dialog.
    Cancelled,
    /// pkexec exit 127: authorization refused, or the helper is not there.
    NotAuthorized,
    /// The helper ran as root and failed; its stderr says why.
    Failed,
}

/// Classify a helper invocation from `pkexec`'s exit code.
///
/// pkexec reserves 126 and 127 for its own failures and otherwise passes the
/// child's status straight through, so anything else means the helper really
/// ran — see pkexec(1).
pub fn classify_failure(exit_code: Option<i32>) -> HelperFailure {
    match exit_code {
        Some(126) => HelperFailure::Cancelled,
        Some(127) => HelperFailure::NotAuthorized,
        _ => HelperFailure::Failed,
    }
}

/// How many trailing lines of the helper's stderr to show.
const STDERR_TAIL_LINES: usize = 6;

/// The dialog body for a failure: the reason, plus the helper's own words when
/// it has any.
pub fn failure_body(failure: HelperFailure, stderr: &str, lang: Lang) -> String {
    let strings = Strings::for_lang(lang);
    let reason = match failure {
        HelperFailure::Cancelled => strings.fix_cancelled_body,
        HelperFailure::NotAuthorized => strings.fix_unauthorized_body,
        HelperFailure::Failed => strings.fix_failed_body,
    };
    match failure {
        // The helper's message is the whole point in this case; in the other
        // two there is nothing on stderr worth showing.
        HelperFailure::Failed => match stderr_tail(stderr) {
            Some(detail) => format!("{reason}\n\n{detail}"),
            None => reason.to_string(),
        },
        _ => reason.to_string(),
    }
}

/// The last few non-empty lines of `stderr`, with control characters removed.
///
/// This text comes from whatever tool the fix ran, so it is no more trusted
/// than a process name or a mount label: strip the C0/C1 range before it
/// reaches a label, exactly as the terminal renderer does.
pub fn stderr_tail(stderr: &str) -> Option<String> {
    let lines: Vec<&str> = stderr
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }
    let start = lines.len().saturating_sub(STDERR_TAIL_LINES);
    Some(
        lines[start..]
            .join("\n")
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect(),
    )
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
///
/// The cut-offs are the **grade** bands from `sysmedic_core::score`, not a
/// second set invented here: with 75/50 a score of 55 was labelled "Poor" and
/// coloured amber, while 60 ("Fair") got the same amber — the colour and the
/// word disagreed about the same number. `score_colour_matches_the_grade`
/// holds the two together.
pub fn score_css(score: u8) -> &'static str {
    match score {
        75..=100 => "success",
        60..=74 => "warning",
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
    fn score_colour_matches_the_grade() {
        // Green for Good/Excellent, amber for Fair, red for Poor/Critical —
        // whatever the grade bands say, so the colour never contradicts the
        // word printed next to it.
        use sysmedic_core::score::grade_for;
        for score in 0..=100u8 {
            let expected = match grade_for(score) {
                "Excellent" | "Good" => "success",
                "Fair" => "warning",
                _ => "error",
            };
            assert_eq!(
                score_css(score),
                expected,
                "score {score} is graded {} but coloured {}",
                grade_for(score),
                score_css(score)
            );
        }
    }

    #[test]
    fn a_dismissed_prompt_is_not_reported_as_a_broken_helper() {
        assert_eq!(classify_failure(Some(126)), HelperFailure::Cancelled);
        assert_eq!(classify_failure(Some(127)), HelperFailure::NotAuthorized);
        // Anything else means the helper ran and exited on its own.
        assert_eq!(classify_failure(Some(1)), HelperFailure::Failed);
        assert_eq!(classify_failure(None), HelperFailure::Failed);

        for lang in [Lang::En, Lang::Ar] {
            let strings = Strings::for_lang(lang);
            assert_eq!(
                failure_body(HelperFailure::Cancelled, "ignored", lang),
                strings.fix_cancelled_body
            );
            assert_eq!(
                failure_body(HelperFailure::NotAuthorized, "", lang),
                strings.fix_unauthorized_body
            );
        }
    }

    #[test]
    fn a_failed_fix_shows_what_the_helper_said() {
        let body = failure_body(
            HelperFailure::Failed,
            "sysmedic-fix-helper: `apt-get autoremove --purge -y` exited with 100: \
             Could not get lock /var/lib/dpkg/lock-frontend\n",
            Lang::En,
        );
        assert!(body.contains("lock-frontend"), "{body}");

        // Nothing on stderr: just the reason, with no trailing blank block.
        let bare = failure_body(HelperFailure::Failed, "   \n\n", Lang::En);
        assert_eq!(bare, Strings::for_lang(Lang::En).fix_failed_body);
    }

    #[test]
    fn helper_output_cannot_smuggle_control_sequences_into_the_dialog() {
        // The text comes from whatever tool the fix ran.
        let tail = stderr_tail("evil\u{1b}[2Jname\nsecond line").unwrap();
        assert!(!tail.contains('\u{1b}'), "{tail:?}");
        assert!(tail.contains("second line"));
        assert!(stderr_tail("").is_none());
        assert!(stderr_tail("\n  \n").is_none());

        // Only the tail is shown, so a chatty tool cannot fill the screen.
        let many: String = (0..40).map(|i| format!("line {i}\n")).collect();
        let tail = stderr_tail(&many).unwrap();
        assert_eq!(tail.lines().count(), 6);
        assert!(tail.contains("line 39"));
        assert!(!tail.contains("line 30"));
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
            trend_since_first,
            undo_last_fix,
            undo_action,
            undo_confirm_title,
            undo_nothing,
            undo_running,
            undo_done,
            undo_failed_title,
            fix_cancelled_body,
            fix_unauthorized_body,
            disk_scanning_path,
            disk_entries,
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
            ("trend_since_first", trend_since_first),
            ("undo_last_fix", undo_last_fix),
            ("undo_action", undo_action),
            ("undo_confirm_title", undo_confirm_title),
            ("undo_nothing", undo_nothing),
            ("undo_running", undo_running),
            ("undo_done", undo_done),
            ("undo_failed_title", undo_failed_title),
            ("fix_cancelled_body", fix_cancelled_body),
            ("fix_unauthorized_body", fix_unauthorized_body),
            ("disk_scanning_path", disk_scanning_path),
            ("disk_entries", disk_entries),
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
