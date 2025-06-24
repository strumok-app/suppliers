use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

pub fn is_allowed(langs: &[String], b: &str) -> bool {
    let langs_alias_once: OnceLock<HashMap<&str, HashSet<&str>>> = OnceLock::new();
    let langs_alias = langs_alias_once.get_or_init(|| {
        HashMap::from([
            ("en", HashSet::from(["eng", "english"])),
            ("uk", HashSet::from(["urk", "ukranian"])),
        ])
    });

    let lowwer_b = b.to_lowercase();

    for l in langs {
        if *l == lowwer_b {
            return true;
        }

        let maybe_alias = langs_alias.get(l.as_str());

        if let Some(alias) = maybe_alias {
            return alias.contains(lowwer_b.as_str());
        }
    }

    false
}
