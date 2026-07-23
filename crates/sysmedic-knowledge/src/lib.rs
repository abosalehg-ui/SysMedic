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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Ar,
}

impl Lang {
    /// Pick a language from a POSIX locale string (`LANG`/`LC_ALL`).
    pub fn from_locale(locale: &str) -> Lang {
        if locale.starts_with("ar") {
            Lang::Ar
        } else {
            Lang::En
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Explanation {
    pub cause: String,
    pub dangerous: String,
    pub impact: String,
    pub remedy: String,
    pub risk_if_ignored: String,
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
