use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

pub fn is_allowed(b: &str) -> bool {
    let langs_alias_once: OnceLock<HashMap<&str, HashSet<&str>>> = OnceLock::new();
    let langs_alias = langs_alias_once.get_or_init(|| {
        HashMap::from([
            ("en", HashSet::from(["eng", "english"])),
            ("uk", HashSet::from(["ukk", "ukranian"])),
            ("ja", HashSet::from(["jpn", "japanese"])),
        ])
    });

    let lowwer_b = b.to_lowercase();

    for l in ["en", "uk", "ja"] {
        if *l == lowwer_b {
            return true;
        }

        let maybe_alias = langs_alias.get(l);

        if let Some(alias) = maybe_alias {
            for a in alias {
                if lowwer_b.contains(a) {
                    return true;
                }
            }
        }
    }

    false
}

/// Normalizes a language name or code (e.g. "english", "eng", "en-US", "ukr", "Ukrainian")
/// into a 2-letter code understood by the app ("en" or "uk").
pub fn normalize(lang: &str) -> Option<String> {
    let lower = lang.trim().to_lowercase();
    let primary = lower
        .split(|c: char| !c.is_alphabetic())
        .find(|s| !s.is_empty())?;

    let code = match primary {
        "en" | "eng" | "english" | "англійська" => "en",
        "uk" | "ua" | "ukr" | "ukrainian" | "українська" => "uk",
        _ => return None,
    };

    Some(code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_normalize_lang() {
        for l in [
            "en",
            "EN",
            "eng",
            "English",
            "en-US",
            "en_GB",
            "English (US)",
        ] {
            assert_eq!(normalize(l).as_deref(), Some("en"), "{l}");
        }
        for l in ["uk", "ua", "ukr", "Ukrainian", "UKR", "uk-UA", "Українська"] {
            assert_eq!(normalize(l).as_deref(), Some("uk"), "{l}");
        }
        for l in ["", "ja", "jpn", "pt-br", "spanish"] {
            assert_eq!(normalize(l), None, "{l}");
        }
    }
}
