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
        serde_yaml_ng::from_str(raw).expect("embedded knowledge.yaml must be valid");
    entries.into_iter().map(|e| (e.id, (e.en, e.ar))).collect()
});

/// Finding ids that were renamed, mapped old → current.
///
/// The ids are documented as stable machine identifiers and appear in
/// `sysmedic explain <id>`, so renaming two of them to match their category
/// (`packages.security_updates` sat in the Security category;
/// `snap.old_revisions` in Storage) would otherwise have broken any script or
/// bookmark using the old name. Lookups accept both; only the new name is
/// emitted.
const RENAMED_IDS: &[(&str, &str)] = &[
    ("packages.security_updates", "security.updates_pending"),
    ("snap.old_revisions", "storage.snap_old_revisions"),
];

/// Resolve an id through the rename table.
pub fn canonical_id(finding_id: &str) -> &str {
    RENAMED_IDS
        .iter()
        .find(|(old, _)| *old == finding_id)
        .map(|(_, new)| *new)
        .unwrap_or(finding_id)
}

/// Explanation for a finding id, or `None` for an unknown id.
/// Accepts pre-rename ids (see [`RENAMED_IDS`]).
pub fn explain(finding_id: &str, lang: Lang) -> Option<&'static Explanation> {
    KNOWLEDGE
        .get(canonical_id(finding_id))
        .map(|(en, ar)| match lang {
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
///
/// Each value is wrapped in bidi isolates (see [`sysmedic_core::lang::isolate`]).
/// The templates are Arabic sentences and the values are machine text —
/// `/boot/efi`, `nvme0n1`, `x86_pkg_temp`, bare numbers — which the bidi
/// algorithm otherwise reorders against the surrounding RTL run, so
/// `نظام الملفات /boot/efi ممتلئ` rendered with the path scrambled. Isolating
/// each value fixes every surface at once (GUI, terminal, HTML, Markdown),
/// because they all render through this one function.
fn render_template(template: &str, args: &[String]) -> String {
    let mut out = template.to_string();
    for (i, arg) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), &sysmedic_core::lang::isolate(arg));
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
    fn interpolated_values_are_bidi_isolated_in_arabic() {
        use sysmedic_core::{Category, Finding, Severity};
        // A mount point is a run of neutral and Latin characters; dropped raw
        // into an RTL sentence the bidi algorithm reorders it on screen.
        let f = Finding::new(
            "storage.disk_nearly_full",
            Category::Storage,
            Severity::Medium,
            "Filesystem /boot/efi is 88% full",
            "Only 0.1 GiB free of 0.5 GiB on /boot/efi.",
        )
        .with_args(vec![
            "/boot/efi".into(),
            "88".into(),
            "0.1".into(),
            "0.5".into(),
        ]);
        let ar = localized_title(&f, Lang::Ar);
        assert!(
            ar.contains("\u{2068}/boot/efi\u{2069}"),
            "path is not isolated: {ar}"
        );
        // Every value gets isolated, not just the first.
        assert_eq!(
            ar.matches('\u{2068}').count(),
            ar.matches('\u{2069}').count()
        );

        // English is authored in the rules and must stay byte-for-byte clean —
        // JSON consumers and the machine-readable title must not grow
        // invisible formatting characters.
        let en = localized_title(&f, Lang::En);
        assert!(!en.contains('\u{2068}') && !en.contains('\u{2069}'));
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
    fn renamed_ids_still_resolve() {
        // Old ids keep working so `sysmedic explain packages.security_updates`
        // in someone's script does not start failing after the rename.
        for (old, new) in super::RENAMED_IDS {
            assert_eq!(canonical_id(old), *new);
            let by_old = explain(old, Lang::En).expect("old id still resolves");
            let by_new = explain(new, Lang::En).expect("new id resolves");
            assert_eq!(by_old.remedy, by_new.remedy);
            // And the Arabic side too.
            assert!(explain(old, Lang::Ar).is_some());
        }
        // An unrelated id passes through untouched.
        assert_eq!(canonical_id("cpu.high_load"), "cpu.high_load");
    }

    #[test]
    fn renamed_ids_point_at_ids_the_rules_actually_emit() {
        // A rename table entry pointing at a nonexistent id would silently
        // resurrect the bug it was meant to fix.
        for (_, new) in super::RENAMED_IDS {
            assert!(
                sysmedic_diagnostics::FINDING_IDS.contains(new),
                "rename target {new} is not a declared finding id"
            );
        }
    }

    #[test]
    fn locale_detection() {
        assert_eq!(Lang::from_locale("ar_SA.UTF-8"), Lang::Ar);
        assert_eq!(Lang::from_locale("en_US.UTF-8"), Lang::En);
        assert_eq!(Lang::from_locale("C"), Lang::En);
    }
}
