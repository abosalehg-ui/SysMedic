//! Language selection for user-facing text the core produces.
//!
//! Lives in the core (rather than the knowledge base) because the domain
//! model itself renders user-facing text: category labels, grades, severity
//! labels and the fix-plan preview a user must read before consenting to a
//! privileged change. `sysmedic-knowledge` re-exports this type, so both
//! layers speak the same language value.

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

/// A short piece of user-facing text carried in both supported languages.
///
/// Used where the text is *generated* rather than looked up — chiefly fix-plan
/// titles and descriptions, which interpolate live numbers ("frees about
/// 2.4 GiB") and so cannot come from a static catalogue. The consent preview
/// for a privileged, sometimes irreversible change is the one screen a user
/// must be able to read in their own language, so the plan carries both rather
/// than leaving the substance English and translating only the labels around it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LocalizedText {
    pub en: String,
    pub ar: String,
}

impl LocalizedText {
    pub fn new(en: impl Into<String>, ar: impl Into<String>) -> Self {
        LocalizedText {
            en: en.into(),
            ar: ar.into(),
        }
    }

    /// The text in `lang`.
    pub fn get(&self, lang: Lang) -> &str {
        match lang {
            Lang::En => &self.en,
            Lang::Ar => &self.ar,
        }
    }
}

#[cfg(test)]
mod localized_tests {
    use super::*;

    #[test]
    fn returns_the_requested_language() {
        let t = LocalizedText::new("Enable the firewall", "تفعيل الجدار الناري");
        assert_eq!(t.get(Lang::En), "Enable the firewall");
        assert_eq!(t.get(Lang::Ar), "تفعيل الجدار الناري");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_language_from_locale() {
        assert_eq!(Lang::from_locale("ar_SA.UTF-8"), Lang::Ar);
        assert_eq!(Lang::from_locale("en_US.UTF-8"), Lang::En);
        assert_eq!(Lang::from_locale("C"), Lang::En);
        assert_eq!(Lang::from_locale(""), Lang::En);
    }
}
