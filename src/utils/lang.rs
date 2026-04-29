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
