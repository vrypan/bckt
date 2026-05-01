use std::collections::BTreeMap;

use isolang::Language;

use crate::config::SearchLanguageConfig;

pub(crate) fn build_lookup(languages: &[SearchLanguageConfig]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for entry in languages {
        let canonical = sanitize_language(&entry.id);
        if canonical.is_empty() {
            continue;
        }

        map.insert(canonical.clone(), entry.id.clone());
        for alias in language_aliases(&canonical) {
            map.entry(alias).or_insert_with(|| entry.id.clone());
        }
    }
    map
}

pub(crate) fn language_aliases(id: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    let primary = id.split('-').next().unwrap_or(id);

    let language = match primary.len() {
        2 => Language::from_639_1(primary),
        3 => Language::from_639_3(primary),
        _ => None,
    };

    if let Some(lang) = language {
        if let Some(code) = lang.to_639_1() {
            aliases.push(code.to_lowercase());
        }
        aliases.push(lang.to_639_3().to_lowercase());
    }

    aliases
}

pub(crate) fn canonical_language(value: &str, map: &BTreeMap<String, String>) -> Option<String> {
    let sanitized = sanitize_language(value);
    if sanitized.is_empty() {
        return None;
    }

    if let Some(found) = map.get(&sanitized) {
        return Some(found.clone());
    }

    if let Some((primary, _rest)) = sanitized.split_once('-')
        && let Some(found) = map.get(primary)
    {
        return Some(found.clone());
    }

    Some(sanitized)
}

pub(crate) fn sanitize_language(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}
