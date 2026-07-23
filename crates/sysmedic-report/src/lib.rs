//! Render a [`HealthReport`] as JSON, Markdown or a standalone HTML page.
//! (PDF arrives in M5 by printing the HTML report.)

use std::fmt::Write as _;

use sysmedic_core::HealthReport;
use sysmedic_knowledge::{explain, Lang};

pub fn to_json(report: &HealthReport) -> String {
    serde_json::to_string_pretty(report).expect("HealthReport serializes")
}

pub fn to_markdown(report: &HealthReport, lang: Lang) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# SysMedic Health Report\n");
    let _ = writeln!(out, "*Generated: {}*\n", report.generated_at);
    let _ = writeln!(
        out,
        "## Health score: **{}/100** ({})\n",
        report.score, report.grade
    );
    let _ = writeln!(out, "| Category | Score |");
    let _ = writeln!(out, "|---|---|");
    for cs in &report.category_scores {
        let _ = writeln!(out, "| {} | {} |", cs.category.label(), cs.score);
    }
    let _ = writeln!(out, "\n## Findings ({})\n", report.findings.len());
    if report.findings.is_empty() {
        let _ = writeln!(out, "No problems found. The system looks healthy.");
    }
    for f in &report.findings {
        let _ = writeln!(
            out,
            "### [{}] {}\n",
            f.severity.label().to_uppercase(),
            f.title
        );
        let _ = writeln!(out, "{}\n", f.summary);
        if let Some(exp) = explain(&f.id, lang) {
            let _ = writeln!(out, "- **Cause:** {}", exp.cause);
            let _ = writeln!(out, "- **Dangerous?** {}", exp.dangerous);
            let _ = writeln!(out, "- **Impact:** {}", exp.impact);
            let _ = writeln!(out, "- **Remedy:** {}", exp.remedy);
            let _ = writeln!(out, "- **If ignored:** {}", exp.risk_if_ignored);
        }
        if !f.evidence.is_empty() {
            let _ = writeln!(out, "\nEvidence:\n");
            for e in &f.evidence {
                let _ = writeln!(out, "- `{e}`");
            }
        }
        if let Some(hint) = &f.fix_hint {
            let _ = writeln!(out, "\nSuggested command: `{hint}`");
        }
        let _ = writeln!(out);
    }
    out
}

pub fn to_html(report: &HealthReport, lang: Lang) -> String {
    let dir = if lang == Lang::Ar { "rtl" } else { "ltr" };
    let mut findings_html = String::new();
    for f in &report.findings {
        let explanation = explain(&f.id, lang)
            .map(|exp| {
                format!(
                    "<ul><li><b>Cause:</b> {}</li><li><b>Dangerous?</b> {}</li>\
                     <li><b>Impact:</b> {}</li><li><b>Remedy:</b> {}</li>\
                     <li><b>If ignored:</b> {}</li></ul>",
                    esc(&exp.cause),
                    esc(&exp.dangerous),
                    esc(&exp.impact),
                    esc(&exp.remedy),
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
<html dir="{dir}"><head><meta charset="utf-8"><title>SysMedic Report</title><style>
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
<h1>SysMedic Health Report</h1>
<p><i>Generated: {generated}</i></p>
<div class="score">{score}/100 <small>({grade})</small></div>
<h2>Categories</h2>{categories}
<h2>Findings ({count})</h2>{findings}
</body></html>"#,
        generated = esc(&report.generated_at),
        score = report.score,
        grade = report.grade,
        count = report.findings.len(),
        findings = findings_html,
    )
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
    fn html_is_escaped_and_rtl_aware() {
        let html = to_html(&report(), Lang::Ar);
        assert!(html.contains("dir=\"rtl\""));
        assert!(html.contains("SysMedic Health Report"));
    }
}
