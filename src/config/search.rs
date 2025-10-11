use std::collections::HashSet;
use std::path::Path;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SearchConfig {
    pub asset_path: String,
    pub default_language: String,
    #[serde(default = "default_search_languages")]
    pub languages: Vec<SearchLanguageConfig>,
    #[serde(default)]
    pub payload_fields: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SearchLanguageConfig {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub stopwords: Vec<String>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            asset_path: "assets/search/search-index.json".to_string(),
            default_language: "en".to_string(),
            languages: default_search_languages(),
            payload_fields: Vec::new(),
        }
    }
}

pub fn validate_search_config(config: &SearchConfig, origin: &Path) -> Result<()> {
    if config.asset_path.trim().is_empty() {
        bail!("{}: search.asset_path must not be empty", origin.display());
    }

    if config.languages.is_empty() {
        bail!(
            "{}: search.languages must define at least one language",
            origin.display()
        );
    }

    let mut seen = HashSet::new();
    for language in &config.languages {
        let key = language.id.trim().to_ascii_lowercase();
        if key.is_empty() {
            bail!(
                "{}: search language ids must not be empty",
                origin.display()
            );
        }
        if !seen.insert(key) {
            bail!(
                "{}: duplicate language id '{}' in search.languages",
                origin.display(),
                language.id
            );
        }
    }

    let default = config.default_language.trim().to_ascii_lowercase();
    if default.is_empty() {
        bail!(
            "{}: search.default_language must not be empty",
            origin.display()
        );
    }

    if !seen.contains(&default) {
        bail!(
            "{}: search.default_language '{}' not found in search.languages",
            origin.display(),
            config.default_language
        );
    }

    let mut payload_seen = HashSet::new();
    for field in &config.payload_fields {
        let trimmed = field.trim();
        if trimmed.is_empty() {
            bail!(
                "{}: search.payload_fields entries must not be empty",
                origin.display()
            );
        }
        if trimmed != field {
            bail!(
                "{}: search.payload_fields '{}' must not contain leading or trailing whitespace",
                origin.display(),
                field
            );
        }
        if trimmed.chars().any(char::is_whitespace) {
            bail!(
                "{}: search.payload_fields '{}' must not contain internal whitespace",
                origin.display(),
                field
            );
        }
        if !payload_seen.insert(trimmed.to_string()) {
            bail!(
                "{}: duplicate entry '{}' in search.payload_fields",
                origin.display(),
                field
            );
        }
    }

    Ok(())
}

fn default_search_languages() -> Vec<SearchLanguageConfig> {
    vec![
        SearchLanguageConfig {
            id: "en".to_string(),
            name: Some("English".to_string()),
            stopwords: default_english_stopwords(),
        },
        SearchLanguageConfig {
            id: "el".to_string(),
            name: Some("Greek".to_string()),
            stopwords: default_greek_stopwords(),
        },
    ]
}

fn default_english_stopwords() -> Vec<String> {
    vec![
        "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "from", "has", "have", "in",
        "is", "it", "of", "on", "or", "that", "the", "to", "was", "were", "will", "with", "you",
        "your", "about", "into", "more", "can", "do", "just", "like", "not", "only", "out", "some",
        "than", "then", "there", "this", "up", "what", "when", "who", "why",
    ]
    .into_iter()
    .map(|word| word.to_string())
    .collect()
}

fn default_greek_stopwords() -> Vec<String> {
    vec![
        "και",
        "να",
        "σε",
        "το",
        "η",
        "ο",
        "οι",
        "τα",
        "για",
        "με",
        "που",
        "ως",
        "από",
        "αυτο",
        "αυτά",
        "αυτή",
        "αυτό",
        "αυτές",
        "αυτοί",
        "αυτών",
        "είναι",
        "στο",
        "στη",
        "στην",
        "στον",
        "τους",
        "τις",
        "των",
        "μια",
        "μιας",
        "μιαν",
        "μου",
        "σου",
        "του",
        "της",
        "μας",
        "σας",
        "αν",
        "θα",
        "δε",
        "δεν",
        "πως",
        "ότι",
        "όπως",
        "όταν",
        "όσο",
    ]
    .into_iter()
    .map(|word| word.to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_search_configuration_has_expected_languages() {
        let config = SearchConfig::default();
        assert_eq!(config.asset_path, "assets/search/search-index.json");
        assert_eq!(config.default_language, "en");
        let ids: Vec<_> = config
            .languages
            .iter()
            .map(|lang| lang.id.as_str())
            .collect();
        assert!(ids.contains(&"en"));
        assert!(ids.contains(&"el"));
        assert!(config.payload_fields.is_empty());
    }

    #[test]
    fn duplicate_search_languages_are_rejected() {
        let mut config = SearchConfig::default();
        config.languages.push(SearchLanguageConfig {
            id: "en".to_string(),
            name: None,
            stopwords: Vec::new(),
        });

        let error = validate_search_config(&config, Path::new("config.yml")).unwrap_err();
        assert!(error.to_string().contains("duplicate language id"));
    }

    #[test]
    fn payload_fields_reject_whitespace_and_duplicates() {
        let config = SearchConfig {
            payload_fields: vec!["image".into(), "image ".into()],
            ..SearchConfig::default()
        };
        let error = validate_search_config(&config, Path::new("config.yml")).unwrap_err();
        assert!(error.to_string().contains("whitespace"));

        let config = SearchConfig {
            payload_fields: vec!["cover".into(), "cover".into()],
            ..SearchConfig::default()
        };
        let error = validate_search_config(&config, Path::new("config.yml")).unwrap_err();
        assert!(error.to_string().contains("duplicate entry"));
    }
}
