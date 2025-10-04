use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use time::UtcOffset;
use time::format_description::{self, FormatItem};
use url::Url;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub title: Option<String>,
    pub base_url: String,
    pub homepage_posts: usize,
    pub date_format: String,
    pub paginate_tags: bool,
    pub default_timezone: String,
    pub theme: Option<String>,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, JsonValue>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SearchConfig {
    pub asset_path: String,
    pub default_language: String,
    #[serde(default = "default_search_languages")]
    pub languages: Vec<SearchLanguageConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SearchLanguageConfig {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub stopwords: Vec<String>,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file {}", path.display()))?;
        let config: Config =
            serde_yaml::from_str(&raw).with_context(|| invalid_yaml_message(path))?;
        config.validate(path)?;
        Ok(config)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let yaml = serde_yaml::to_string(self)?;
        fs::write(path, yaml)
            .with_context(|| format!("failed to write config file {}", path.display()))?;
        Ok(())
    }

    pub fn validate(&self, origin: &Path) -> Result<()> {
        validate_url(&self.base_url, origin)?;
        if self.homepage_posts == 0 {
            bail!(
                "{}: homepage_posts must be greater than zero",
                origin.display()
            );
        }
        validate_format(&self.date_format, origin)?;
        validate_timezone(&self.default_timezone, origin)?;
        validate_search_config(&self.search, origin)?;
        Ok(())
    }

    pub fn default_offset(&self) -> Result<UtcOffset> {
        parse_timezone(&self.default_timezone)
    }
}

pub fn find_project_root(start: impl AsRef<Path>) -> Result<PathBuf> {
    let mut current = start.as_ref().to_path_buf();

    loop {
        let candidate = current.join("bckt.yaml");
        if candidate.exists() {
            return Ok(current);
        }

        if !current.pop() {
            bail!(
                "could not locate bckt.yaml starting from {}",
                start.as_ref().display()
            );
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            title: None,
            base_url: "https://example.com".to_string(),
            homepage_posts: 5,
            date_format: "[year]-[month]-[day]".to_string(),
            paginate_tags: true,
            default_timezone: "+00:00".to_string(),
            theme: Some("bckt3".to_string()),
            search: SearchConfig::default(),
            extra: serde_json::Map::new(),
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            asset_path: "assets/search/search-index.json".to_string(),
            default_language: "en".to_string(),
            languages: default_search_languages(),
        }
    }
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

fn invalid_yaml_message(path: &Path) -> String {
    format!("{}: invalid YAML", path.display())
}

fn validate_url(value: &str, origin: &Path) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{}: base_url must not be empty", origin.display());
    }
    let url = Url::parse(value)
        .with_context(|| format!("{}: base_url must be an absolute URL", origin.display()))?;
    if !matches!(url.scheme(), "http" | "https") {
        bail!("{}: base_url must use http or https", origin.display());
    }
    Ok(())
}

fn validate_format(value: &str, origin: &Path) -> Result<()> {
    parse_format(value).with_context(|| {
        format!(
            "{}: date_format '{}' is invalid (see https://docs.rs/time/latest/time/format_description)",
            origin.display(), value
        )
    })?;
    Ok(())
}

fn validate_timezone(value: &str, origin: &Path) -> Result<()> {
    parse_timezone(value).with_context(|| {
        format!(
            "{}: default_timezone '{}' is invalid (expected offset like +00:00)",
            origin.display(),
            value
        )
    })?;
    Ok(())
}

fn validate_search_config(config: &SearchConfig, origin: &Path) -> Result<()> {
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

    Ok(())
}

fn parse_timezone(value: &str) -> Result<UtcOffset> {
    if value.eq_ignore_ascii_case("UTC") || value.eq_ignore_ascii_case("Z") {
        return Ok(UtcOffset::UTC);
    }

    let trimmed = value.trim();
    let mut chars = trimmed.chars();
    let sign_char = chars
        .next()
        .with_context(|| format!("default_timezone '{}' is empty", value))?;
    let sign = match sign_char {
        '+' => 1,
        '-' => -1,
        _ => bail!("default_timezone must start with '+' or '-'"),
    };

    let remainder = chars.as_str();
    let mut parts = remainder.split(':');
    let hours_str = parts
        .next()
        .with_context(|| format!("default_timezone '{}' missing hour component", value))?;
    let minutes_str = parts.next().unwrap_or("0");
    let seconds_str = parts.next().unwrap_or("0");

    if parts.next().is_some() {
        bail!("default_timezone '{}' has too many components", value);
    }

    let hours: i8 = hours_str
        .parse()
        .with_context(|| format!("default_timezone '{}' hour component invalid", value))?;
    let minutes: i8 = minutes_str
        .parse()
        .with_context(|| format!("default_timezone '{}' minute component invalid", value))?;
    let seconds: i8 = seconds_str
        .parse()
        .with_context(|| format!("default_timezone '{}' second component invalid", value))?;

    UtcOffset::from_hms(sign * hours, sign * minutes, sign * seconds)
        .with_context(|| format!("default_timezone '{}' out of range", value))
}

fn parse_format(value: &str) -> Result<()> {
    if value == "RFC3339" {
        return Ok(());
    }

    let items = format_description::parse(value)?;
    if !items
        .iter()
        .any(|item| matches!(item, FormatItem::Component(_)))
    {
        bail!("date_format must contain at least one date or time component");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn default_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bckt.yaml");
        let config = Config::load(&path).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn load_valid_config() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bckt.yaml");
        fs::write(
            &path,
            r#"title: "Bucket"
base_url: "https://example.com/blog"
homepage_posts: 8
paginate_tags: false
default_timezone: "+05:30"
"#,
        )
        .unwrap();

        let config = Config::load(&path).unwrap();
        assert_eq!(config.title.as_deref(), Some("Bucket"));
        assert_eq!(config.base_url, "https://example.com/blog");
        assert_eq!(config.homepage_posts, 8);
        assert_eq!(config.date_format, "[year]-[month]-[day]");
        assert!(!config.paginate_tags);
        assert_eq!(config.default_timezone, "+05:30");
        assert_eq!(config.theme.as_deref(), Some("bckt3"));
    }

    #[test]
    fn save_round_trips_config() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bckt.yaml");
        let mut config = Config::default();
        config.title = Some("Saved".into());
        config.theme = Some("bckt3".into());
        config.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.title.as_deref(), Some("Saved"));
        assert_eq!(loaded.theme.as_deref(), Some("bckt3"));
    }

    #[test]
    fn default_search_configuration_has_expected_languages() {
        let config = Config::default();
        assert_eq!(config.search.asset_path, "assets/search/search-index.json");
        assert_eq!(config.search.default_language, "en");
        let ids: Vec<_> = config
            .search
            .languages
            .iter()
            .map(|lang| lang.id.as_str())
            .collect();
        assert!(ids.contains(&"en"));
        assert!(ids.contains(&"el"));
    }

    #[test]
    fn duplicate_search_languages_are_rejected() {
        let mut config = Config::default();
        config.search.languages.push(SearchLanguageConfig {
            id: "en".to_string(),
            name: None,
            stopwords: Vec::new(),
        });

        let error = config.validate(Path::new("config.yml")).unwrap_err();
        assert!(error.to_string().contains("duplicate language id"));
    }

    #[test]
    fn find_project_root_walks_upwards() {
        let dir = TempDir::new().unwrap();
        let project = dir.path();
        let nested = project.join("posts/example");
        fs::create_dir_all(&nested).unwrap();
        fs::write(project.join("bckt.yaml"), "title: test\n").unwrap();

        let discovered = find_project_root(&nested).unwrap();
        assert_eq!(discovered, project);
    }

    #[test]
    fn find_project_root_errors_when_missing() {
        let dir = TempDir::new().unwrap();
        let error = find_project_root(dir.path()).unwrap_err();
        assert!(error.to_string().contains("could not locate bckt.yaml"));
    }

    #[test]
    fn reject_invalid_url() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bckt.yaml");
        fs::write(
            &path,
            r#"title: "Bucket"
base_url: "ftp://example.com"
homepage_posts: 3
"#,
        )
        .unwrap();

        let error = Config::load(&path).unwrap_err();
        let message = format!("{error}");
        assert!(message.contains("base_url must use http or https"));
    }

    #[test]
    fn reject_zero_homepage_posts() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bckt.yaml");
        fs::write(
            &path,
            r#"base_url: "https://example.com"
homepage_posts: 0
"#,
        )
        .unwrap();

        let error = Config::load(&path).unwrap_err();
        assert!(format!("{error}").contains("homepage_posts must be greater than zero"));
    }

    #[test]
    fn reject_invalid_date_format() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bckt.yaml");
        fs::write(
            &path,
            r#"base_url: "https://example.com"
date_format: "???"
"#,
        )
        .unwrap();

        let error = Config::load(&path).unwrap_err();
        assert!(format!("{error}").contains("date_format"));
    }

    #[test]
    fn accept_rfc3339_keyword() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bckt.yaml");
        fs::write(
            &path,
            r#"base_url: "https://example.com"
date_format: "RFC3339"
"#,
        )
        .unwrap();

        let config = Config::load(&path).unwrap();
        assert_eq!(config.date_format, "RFC3339");
    }

    #[test]
    fn reject_invalid_timezone() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bckt.yaml");
        fs::write(
            &path,
            r#"base_url: "https://example.com"
default_timezone: "Mars/Station"
"#,
        )
        .unwrap();

        let error = Config::load(&path).unwrap_err();
        assert!(format!("{error}").contains("default_timezone"));
    }
}
