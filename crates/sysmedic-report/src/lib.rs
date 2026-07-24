//! Render a [`HealthReport`] as JSON, Markdown, a standalone HTML page, or a
//! PDF (by printing the HTML through a headless browser).

use std::fmt::Write as _;
use std::path::Path;

use sysmedic_core::HealthReport;
use sysmedic_knowledge::{explain, Lang};

pub fn to_json(report: &HealthReport) -> String {
    serde_json::to_string_pretty(report).expect("HealthReport serializes")
}

/// Headless-browser / converter candidates tried in order for PDF export.
const PDF_TOOLS: &[(&str, &[&str])] = &[
    (
        "chromium",
        &["--headless", "--no-sandbox", "--print-to-pdf={out}", "{in}"],
    ),
    (
        "chromium-browser",
        &["--headless", "--no-sandbox", "--print-to-pdf={out}", "{in}"],
    ),
    (
        "google-chrome",
        &["--headless", "--no-sandbox", "--print-to-pdf={out}", "{in}"],
    ),
    ("wkhtmltopdf", &["{in}", "{out}"]),
];

/// Write a PDF of `report` to `out_path` by rendering HTML and converting it
/// with whatever headless browser / wkhtmltopdf is installed. Returns an error
/// (naming the options) when none is available, so the caller can fall back to
/// HTML.
pub fn write_pdf(report: &HealthReport, lang: Lang, out_path: &Path) -> Result<(), String> {
    let html = to_html(report, lang);
    // A private, uniquely-named temp file (O_EXCL, mode 0600) that auto-deletes
    // when dropped at the end of this function. This avoids the fixed,
    // predictable, world-readable path in the shared temp dir, which was open
    // to a symlink-overwrite attack and leaked the full report to other local
    // users. The handle is kept alive across the conversion below.
    let mut tmp = tempfile::Builder::new()
        .prefix("sysmedic-report-")
        .suffix(".html")
        .tempfile()
        .map_err(|e| format!("cannot create temp HTML: {e}"))?;
    {
        use std::io::Write as _;
        tmp.write_all(html.as_bytes())
            .and_then(|()| tmp.flush())
            .map_err(|e| format!("cannot write temp HTML: {e}"))?;
    }
    let tmp_path = tmp.path().to_path_buf();
    let (tmp_s, out_s) = (tmp_path.to_string_lossy(), out_path.to_string_lossy());

    for (tool, template) in PDF_TOOLS {
        if which(tool).is_none() {
            continue;
        }
        let args: Vec<String> = template
            .iter()
            .map(|a| a.replace("{out}", &out_s).replace("{in}", &tmp_s))
            .collect();
        let ok = std::process::Command::new(tool)
            .args(&args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok && out_path.exists() {
            return Ok(());
        }
    }
    Err("no PDF converter found (install chromium or wkhtmltopdf); \
         the HTML report was produced instead"
        .to_string())
}

fn which(program: &str) -> Option<()> {
    std::process::Command::new(program)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| ())
}

pub fn to_markdown(report: &HealthReport, lang: Lang) -> String {
    let l = labels(lang);
    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", l.report_title);
    let _ = writeln!(
        out,
        "*{}: {}*\n",
        l.generated,
        md_inline(&report.generated_at)
    );
    let _ = writeln!(
        out,
        "## {}: **{}/100** ({})\n",
        l.score, report.score, report.grade
    );
    let _ = writeln!(out, "| {} | |", l.categories);
    let _ = writeln!(out, "|---|---|");
    for cs in &report.category_scores {
        let _ = writeln!(out, "| {} | {} |", cs.category.label(), cs.score);
    }
    let _ = writeln!(out, "\n## {} ({})\n", l.findings, report.findings.len());
    if report.findings.is_empty() {
        let _ = writeln!(out, "{}", l.healthy);
    }
    for f in &report.findings {
        let _ = writeln!(
            out,
            "### [{}] {}\n",
            f.severity.label().to_uppercase(),
            md_inline(&f.title)
        );
        let _ = writeln!(out, "{}\n", md_inline(&f.summary));
        if let Some(exp) = explain(&f.id, lang) {
            let _ = writeln!(out, "- **{}:** {}", l.cause, md_inline(&exp.cause));
            let _ = writeln!(out, "- **{}:** {}", l.dangerous, md_inline(&exp.dangerous));
            let _ = writeln!(out, "- **{}:** {}", l.impact, md_inline(&exp.impact));
            let _ = writeln!(out, "- **{}:** {}", l.remedy, md_inline(&exp.remedy));
            let _ = writeln!(
                out,
                "- **{}:** {}",
                l.if_ignored,
                md_inline(&exp.risk_if_ignored)
            );
        }
        if !f.evidence.is_empty() {
            let _ = writeln!(out, "\n{}:\n", l.evidence);
            for e in &f.evidence {
                // Escaped plain text rather than an inline code span: a backtick
                // in the evidence would otherwise break out of the span.
                let _ = writeln!(out, "- {}", md_inline(e));
            }
        }
        if let Some(hint) = &f.fix_hint {
            // fix hints are static, developer-authored strings (not user input).
            let _ = writeln!(out, "\n{}: `{hint}`", l.suggested);
        }
        let _ = writeln!(out);
    }
    out
}

pub fn to_html(report: &HealthReport, lang: Lang) -> String {
    let l = labels(lang);
    let dir = if lang == Lang::Ar { "rtl" } else { "ltr" };
    let mut findings_html = String::new();
    for f in &report.findings {
        let explanation = explain(&f.id, lang)
            .map(|exp| {
                format!(
                    "<ul><li><b>{}:</b> {}</li><li><b>{}:</b> {}</li>\
                     <li><b>{}:</b> {}</li><li><b>{}:</b> {}</li>\
                     <li><b>{}:</b> {}</li></ul>",
                    esc(l.cause),
                    esc(&exp.cause),
                    esc(l.dangerous),
                    esc(&exp.dangerous),
                    esc(l.impact),
                    esc(&exp.impact),
                    esc(l.remedy),
                    esc(&exp.remedy),
                    esc(l.if_ignored),
                    esc(&exp.risk_if_ignored)
                )
            })
            .unwrap_or_default();
        let evidence = if f.evidence.is_empty() {
            String::new()
        } else {
            format!("<pre>{}</pre>", esc(&f.evidence.join("\n")))
        };
        let _ = write!(
            findings_html,
            "<article class=\"sev-{sev}\"><h3><span class=\"badge\">{sev}</span> {title}</h3>\
             <p>{summary}</p>{explanation}{evidence}</article>",
            sev = f.severity.label(),
            title = esc(&f.title),
            summary = esc(&f.summary),
        );
    }
    let categories: String = report
        .category_scores
        .iter()
        .map(|cs| {
            format!(
                "<div class=\"cat\"><span>{}</span><div class=\"bar\"><div style=\"width:{}%\"></div></div><b>{}</b></div>",
                cs.category.label(),
                cs.score,
                cs.score
            )
        })
        .collect();
    format!(
        r#"<!DOCTYPE html>
<html dir="{dir}"><head><meta charset="utf-8"><title>{report_title}</title><style>
:root {{ color-scheme: light dark; font-family: system-ui, sans-serif; }}
body {{ max-width: 860px; margin: 2rem auto; padding: 0 1rem; }}
.score {{ font-size: 3rem; font-weight: 700; }}
.cat {{ display: grid; grid-template-columns: 8rem 1fr 3rem; gap: .5rem; align-items: center; margin: .2rem 0; }}
.bar {{ background: rgba(128,128,128,.25); border-radius: 6px; height: 10px; }}
.bar div {{ background: #26a269; border-radius: 6px; height: 10px; }}
article {{ border: 1px solid rgba(128,128,128,.35); border-radius: 10px; padding: .2rem 1rem 1rem; margin: 1rem 0; }}
.badge {{ font-size: .7rem; text-transform: uppercase; padding: .15rem .5rem; border-radius: 999px; background: rgba(128,128,128,.25); }}
.sev-critical .badge {{ background: #c01c28; color: #fff; }}
.sev-high .badge {{ background: #e66100; color: #fff; }}
.sev-medium .badge {{ background: #e5a50a; }}
pre {{ overflow-x: auto; background: rgba(128,128,128,.15); padding: .6rem; border-radius: 8px; }}
</style></head><body>
<h1>{report_title}</h1>
<p><i>{generated_label}: {generated}</i></p>
<div class="score">{score}/100 <small>({grade})</small></div>
<h2>{categories_label}</h2>{categories}
<h2>{findings_label} ({count})</h2>{findings}
</body></html>"#,
        report_title = esc(l.report_title),
        generated_label = esc(l.generated),
        generated = esc(&report.generated_at),
        categories_label = esc(l.categories),
        findings_label = esc(l.findings),
        score = report.score,
        grade = report.grade,
        count = report.findings.len(),
        findings = findings_html,
    )
}

/// Escape a string for HTML. Covers element *and* attribute context (quotes
/// included) so a future move of an escaped value into an attribute can't
/// become an injection point.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Escape text for Markdown body/inline context. Finding titles, summaries and
/// evidence contain attacker-influenceable process/file/service names, and
/// Markdown reports are pasted into GitHub issues/wikis where raw inline HTML
/// and Markdown metacharacters are rendered. Backslash-escaping the
/// significant characters makes the content render literally.
fn md_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '<' | '>' | '#' | '|'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Localized labels for the report chrome (headings, field names). The finding
/// *bodies* are already localized via the knowledge base; this localizes the
/// surrounding structure so an Arabic report is not half English.
struct Labels {
    report_title: &'static str,
    generated: &'static str,
    score: &'static str,
    categories: &'static str,
    findings: &'static str,
    healthy: &'static str,
    cause: &'static str,
    dangerous: &'static str,
    impact: &'static str,
    remedy: &'static str,
    if_ignored: &'static str,
    evidence: &'static str,
    suggested: &'static str,
}

fn labels(lang: Lang) -> Labels {
    match lang {
        Lang::Ar => Labels {
            report_title: "تقرير صحّة SysMedic",
            generated: "أُنشئ في",
            score: "درجة الصحّة",
            categories: "الفئات",
            findings: "النتائج",
            healthy: "لا توجد مشكلات — النظام يبدو سليماً.",
            cause: "السبب",
            dangerous: "هل هو خطير؟",
            impact: "التأثير",
            remedy: "العلاج",
            if_ignored: "إذا أُهمل",
            evidence: "الدليل",
            suggested: "أمر مقترح",
        },
        Lang::En => Labels {
            report_title: "SysMedic Health Report",
            generated: "Generated",
            score: "Health score",
            categories: "Categories",
            findings: "Findings",
            healthy: "No problems found. The system looks healthy.",
            cause: "Cause",
            dangerous: "Dangerous?",
            impact: "Impact",
            remedy: "Remedy",
            if_ignored: "If ignored",
            evidence: "Evidence",
            suggested: "Suggested command",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysmedic_core::{Category, Finding, Severity, Snapshot};

    fn report() -> HealthReport {
        HealthReport::build(
            Snapshot::default(),
            vec![Finding::new(
                "storage.disk_nearly_full",
                Category::Storage,
                Severity::Critical,
                "Filesystem / is 96% full",
                "Only 4 GiB free.",
            )],
        )
    }

    #[test]
    fn json_roundtrips() {
        let json = to_json(&report());
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["findings"][0]["id"], "storage.disk_nearly_full");
    }

    #[test]
    fn markdown_includes_explanation() {
        let md = to_markdown(&report(), Lang::En);
        assert!(md.contains("Health score"));
        assert!(md.contains("**Remedy:**"));
    }

    #[test]
    fn html_is_rtl_aware_and_localizes_chrome() {
        let html = to_html(&report(), Lang::Ar);
        assert!(html.contains("dir=\"rtl\""));
        // The report chrome is Arabic, not half-English.
        assert!(html.contains("تقرير صحّة SysMedic"));
        assert!(html.contains("النتائج"));
    }

    #[test]
    fn markdown_localizes_chrome_in_arabic() {
        let md = to_markdown(&report(), Lang::Ar);
        assert!(md.contains("درجة الصحّة"));
        assert!(md.contains("## النتائج"));
    }

    #[test]
    fn html_escapes_injection_in_findings() {
        let report = HealthReport::build(
            Snapshot::default(),
            vec![Finding::new(
                "storage.disk_nearly_full",
                Category::Storage,
                Severity::Critical,
                "<script>alert(1)</script>\" evil",
                "summary",
            )],
        );
        let html = to_html(&report, Lang::En);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&quot;"));
    }

    #[test]
    fn markdown_escapes_injection_in_title_and_evidence() {
        let f = Finding::new(
            "storage.disk_nearly_full",
            Category::Storage,
            Severity::Critical,
            "# Not a heading <b>",
            "s",
        )
        .with_evidence(vec!["`rm -rf` <script>".into()]);
        let report = HealthReport::build(Snapshot::default(), vec![f]);
        let md = to_markdown(&report, Lang::En);
        assert!(md.contains("\\# Not a heading \\<b\\>"));
        assert!(md.contains("\\`rm -rf\\`"));
    }
}
