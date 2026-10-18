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

    /// The short tag for this language (`en`/`ar`), for passing it between
    /// SysMedic's own processes. [`Lang::from_tag`] is its inverse.
    pub fn tag(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ar => "ar",
        }
    }

    /// Parse a [`Lang::tag`]; `None` for anything else. Strict on purpose —
    /// the privileged helper validates its arguments with this.
    pub fn from_tag(tag: &str) -> Option<Lang> {
        match tag {
            "en" => Some(Lang::En),
            "ar" => Some(Lang::Ar),
            _ => None,
        }
    }

    /// The user's language, from the environment.
    ///
    /// POSIX gives message catalogues a precedence order — `LC_ALL` overrides
    /// everything, then `LC_MESSAGES`, then `LANG` — and every entry point
    /// (CLI, GUI, the privileged helper) previously read `LANG` alone, in five
    /// separate copies of the same line. A user whose shell sets
    /// `LC_ALL=ar_SA.UTF-8` on top of an English `LANG` got English output.
    pub fn from_env() -> Lang {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Some(value) = std::env::var(key).ok().filter(|v| !v.is_empty()) {
                return Lang::from_locale(&value);
            }
        }
        Lang::En
    }
}

/// Wrap `value` in Unicode bidi isolates (FSI … PDI) so it keeps its own
/// direction when embedded in text of the opposite one.
///
/// Arabic finding titles interpolate machine values — mount points, device
/// nodes, unit names, numbers. Inside an RTL sentence the bidi algorithm
/// reorders those (a path is a run of neutrals and Latin), so
/// `نظام الملفات /boot/efi ممتلئ` rendered with the path visually mangled.
/// FSI (U+2068) opens a run whose direction is detected from its own first
/// strong character; PDI (U+2069) closes it. Both are formatting characters,
/// not control characters, so terminal sanitizing keeps them and terminals
/// that don't implement bidi ignore them.
pub fn isolate(value: &str) -> String {
    format!("\u{2068}{value}\u{2069}")
}

/// A counted noun in Arabic, with the form the number demands.
///
/// English gets by with "finding(s)"; Arabic does not: one takes the
/// singular, two the dual, three to ten the plural, and eleven upwards the
/// singular again (`5 نتائج` but `15 نتيجة`). The terminal, the history list,
/// the disk page and the fix previews all printed `"{n} {singular}"`, which
/// reads as a mistake to every Arabic user at exactly the numbers that occur
/// most. Digits stay Western, as the typography policy in `knowledge.yaml`
/// requires.
///
/// `dual` is given by the caller in the case the sentence needs
/// (`نتيجتان` as a subject, `حزمتين` as an object).
pub fn count_ar(n: u64, singular: &str, dual: &str, plural: &str) -> String {
    match n {
        1 => format!("1 {singular}"),
        2 => dual.to_string(),
        3..=10 => format!("{n} {plural}"),
        _ => format!("{n} {singular}"),
    }
}

/// A noun that gets counted in both languages, e.g. "finding(s)".
///
/// English needs two forms, Arabic three ([`count_ar`]); carrying them
/// together keeps "5 findings" and "5 نتائج" the same decision in the code.
#[derive(Debug, Clone, Copy)]
pub struct CountedNoun {
    pub en_one: &'static str,
    pub en_many: &'static str,
    pub ar_one: &'static str,
    pub ar_two: &'static str,
    pub ar_many: &'static str,
}

impl CountedNoun {
    /// `n` of this noun, in `lang`.
    pub fn render(&self, n: u64, lang: Lang) -> String {
        match lang {
            Lang::En if n == 1 => format!("1 {}", self.en_one),
            Lang::En => format!("{n} {}", self.en_many),
            Lang::Ar => count_ar(n, self.ar_one, self.ar_two, self.ar_many),
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

    #[test]
    fn arabic_counts_take_the_form_the_number_demands() {
        let f = |n| count_ar(n, "نتيجة", "نتيجتان", "نتائج");
        assert_eq!(f(0), "0 نتيجة");
        assert_eq!(f(1), "1 نتيجة");
        assert_eq!(f(2), "نتيجتان");
        assert_eq!(f(3), "3 نتائج");
        assert_eq!(f(10), "10 نتائج");
        assert_eq!(f(11), "11 نتيجة");
        assert_eq!(f(184320), "184320 نتيجة");
    }

    #[test]
    fn counted_nouns_render_in_both_languages() {
        const FINDINGS: CountedNoun = CountedNoun {
            en_one: "finding",
            en_many: "findings",
            ar_one: "نتيجة",
            ar_two: "نتيجتان",
            ar_many: "نتائج",
        };
        assert_eq!(FINDINGS.render(1, Lang::En), "1 finding");
        assert_eq!(FINDINGS.render(5, Lang::En), "5 findings");
        assert_eq!(FINDINGS.render(0, Lang::En), "0 findings");
        assert_eq!(FINDINGS.render(2, Lang::Ar), "نتيجتان");
        assert_eq!(FINDINGS.render(5, Lang::Ar), "5 نتائج");
    }

    #[test]
    fn tags_round_trip_and_reject_everything_else() {
        for lang in [Lang::En, Lang::Ar] {
            assert_eq!(Lang::from_tag(lang.tag()), Some(lang));
        }
        assert_eq!(Lang::from_tag("fr"), None);
        assert_eq!(Lang::from_tag("AR"), None);
        assert_eq!(Lang::from_tag(""), None);
    }

    #[test]
    fn isolate_wraps_in_fsi_pdi() {
        assert_eq!(isolate("/boot/efi"), "\u{2068}/boot/efi\u{2069}");
        // The isolates are format characters, not control characters, so the
        // terminal sanitizer keeps them.
        assert!(!'\u{2068}'.is_control());
        assert!(!'\u{2069}'.is_control());
    }
}
