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
