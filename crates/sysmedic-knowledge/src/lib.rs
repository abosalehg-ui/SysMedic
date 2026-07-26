//! The SysMedic explanation knowledge base.
//!
//! For every finding id the base answers, offline and in both English and
//! Arabic: what caused it, whether it is dangerous, what the impact is,
//! how to fix it, and what happens if it is ignored. An optional
//! LLM-backed [`Explainer`] can layer deeper, context-aware explanations
//! on top from M6 onwards.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::Deserialize;

pub mod llm;
pub use llm::{HttpTransport, LlmExplainer, DEFAULT_MODEL};

// The language enum lives in the core so the domain model (categories, grades,
// fix previews) and the knowledge base agree on one type.
pub use sysmedic_core::Lang;

#[derive(Debug, Clone, Deserialize)]
pub struct Explanation {
    pub cause: String,
    pub dangerous: String,
    pub impact: String,
    pub remedy: String,
    pub risk_if_ignored: String,
    /// Template for the finding *title* in this language, with `{0}`, `{1}`…
    /// placeholders for the finding's `args`. Present for translations; the
    /// English title always comes from the rule itself.
    #[serde(default)]
    pub title: Option<String>,
    /// Template for the finding *summary*, same placeholder scheme.
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    id: String,
    en: Explanation,
    ar: Explanation,
}

static KNOWLEDGE: Lazy<HashMap<String, (Explanation, Explanation)>> = Lazy::new(|| {
    let raw = include_str!("../data/knowledge.yaml");
    let entries: Vec<Entry> =
        serde_yaml::from_str(raw).expect("embedded knowledge.yaml must be valid");
    entries.into_iter().map(|e| (e.id, (e.en, e.ar))).collect()
});

/// Explanation for a finding id, or `None` for an unknown id.
pub fn explain(finding_id: &str, lang: Lang) -> Option<&'static Explanation> {
    KNOWLEDGE.get(finding_id).map(|(en, ar)| match lang {
        Lang::En => en,
        Lang::Ar => ar,
    })
}

/// The finding's title in `lang`, rendered from the per-id template with the
/// finding's `args` substituted for `{0}`, `{1}`, … Falls back to the rule's
/// own (English) title when no template exists for the id/language — a
/// missing translation must never hide a finding.
pub fn localized_title(finding: &sysmedic_core::Finding, lang: Lang) -> String {
    localized_field(finding, lang, |e| e.title.as_deref(), &finding.title)
}

/// The finding's summary in `lang` (same mechanism as [`localized_title`]).
pub fn localized_summary(finding: &sysmedic_core::Finding, lang: Lang) -> String {
    localized_field(finding, lang, |e| e.summary.as_deref(), &finding.summary)
}

fn localized_field(
    finding: &sysmedic_core::Finding,
    lang: Lang,
    pick: impl Fn(&'static Explanation) -> Option<&'static str>,
    fallback: &str,
) -> String {
    if lang == Lang::En {
        // English is authored in the rules; templates exist for translations.
        return fallback.to_string();
    }
    explain(&finding.id, lang)
        .and_then(&pick)
        .map(|template| render_template(template, &finding.args))
        .unwrap_or_else(|| fallback.to_string())
}

/// Substitute `{0}`, `{1}`, … with `args`. Unknown or out-of-range
/// placeholders are left verbatim so a template/args mismatch is visible in
/// tests instead of silently dropping data.
fn render_template(template: &str, args: &[String]) -> String {
    let mut out = template.to_string();
    for (i, arg) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), arg);
    }
    out
}

/// Pluggable deep-explanation backend (LLM providers implement this in M6).
pub trait Explainer: Send + Sync {
    fn explain(&self, finding_id: &str, context: &str, lang: Lang) -> Option<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knowledge_base_parses() {
        assert!(!KNOWLEDGE.is_empty());
    }

    #[test]
    fn renders_templates_and_falls_back() {
        use sysmedic_core::{Category, Finding, Severity};
        let f = Finding::new(
            "storage.disk_nearly_full",
            Category::Storage,
            Severity::Medium,
            "Filesystem / is 88% full",
            "Only 29.6 GiB free of 252.0 GiB on /.",
        )
        .with_args(vec!["/".into(), "88".into(), "29.6".into(), "252.0".into()]);
        let ar = localized_title(&f, Lang::Ar);
        assert!(ar.contains('/') && ar.contains("88"), "got: {ar}");
        assert!(!ar.contains('{'), "unfilled placeholder in: {ar}");
        assert!(!localized_summary(&f, Lang::Ar).contains('{'));
        // English always comes from the rule text itself.
        assert_eq!(localized_title(&f, Lang::En), f.title);
        // Unknown ids fall back rather than disappearing.
        let unknown = Finding::new("x.y", Category::Cpu, Severity::Low, "T", "S");
        assert_eq!(localized_title(&unknown, Lang::Ar), "T");
    }

    #[test]
    fn every_finding_has_arabic_title_and_summary_templates() {
        for id in sysmedic_diagnostics::FINDING_IDS {
            let ar = explain(id, Lang::Ar).unwrap_or_else(|| panic!("missing ar entry for {id}"));
            let title = ar.title.as_deref().unwrap_or_default();
            let summary = ar.summary.as_deref().unwrap_or_default();
            assert!(!title.is_empty(), "missing Arabic title template for {id}");
            assert!(
                !summary.is_empty(),
                "missing Arabic summary template for {id}"
            );
            // Rendering with four args must satisfy every placeholder — no
            // rule passes more than four, so an unfilled `{n}` here means a
            // template references an argument that will never exist.
            let probe: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
            for template in [title, summary] {
                let rendered = super::render_template(template, &probe);
                assert!(
                    !rendered.contains('{'),
                    "template for {id} references an out-of-range arg: {template}"
                );
            }
        }
    }

    #[test]
    fn every_diagnostic_id_has_bilingual_explanation() {
        for id in sysmedic_diagnostics::FINDING_IDS {
            let en = explain(id, Lang::En);
            let ar = explain(id, Lang::Ar);
            assert!(en.is_some(), "missing English explanation for {id}");
            assert!(ar.is_some(), "missing Arabic explanation for {id}");
            assert!(!en.unwrap().remedy.is_empty());
            assert!(!ar.unwrap().remedy.is_empty());
        }
    }

    #[test]
    fn locale_detection() {
        assert_eq!(Lang::from_locale("ar_SA.UTF-8"), Lang::Ar);
        assert_eq!(Lang::from_locale("en_US.UTF-8"), Lang::En);
        assert_eq!(Lang::from_locale("C"), Lang::En);
    }
}
