use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use blake3::Hasher;
use isolang::Language;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description;
use time::format_description::well_known::Rfc3339;

use crate::config::{Config, SearchLanguageConfig};
use crate::content::Post;

#[derive(Debug)]
pub struct SearchIndexArtifact {
    pub bytes: Vec<u8>,
    pub digest: String,
    pub document_count: usize,
}

#[derive(Serialize)]
struct SearchIndex {
    version: u8,
    generated_at: String,
    default_language: String,
    languages: Vec<SearchLanguageMeta>,
    documents: Vec<SearchDocument>,
    facets: SearchFacets,
}

#[derive(Serialize)]
struct SearchLanguageMeta {
    id: String,
    name: Option<String>,
    stopwords: Vec<String>,
}

#[derive(Serialize)]
struct SearchDocument {
    id: String,
    title: String,
    url: String,
    language: String,
    tags: Vec<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    date_display: String,
    date_iso: String,
    timestamp: i64,
    excerpt: String,
    content: String,
}

#[derive(Serialize)]
struct SearchFacets {
    tags: Vec<String>,
    #[serde(rename = "types")]
    types: Vec<String>,
    years: Vec<i32>,
}

pub fn build_index(config: &Config, posts: &[Post]) -> Result<SearchIndexArtifact> {
    let now = OffsetDateTime::now_utc();
    let generated_at = now
        .format(&Rfc3339)
        .context("failed to format generated_at timestamp")?;

    let language_lookup = language_lookup(&config.search.languages);
    let default_language = canonical_language(&config.search.default_language, &language_lookup)
        .unwrap_or_else(|| sanitize_language(&config.search.default_language));

    let mut documents = Vec::with_capacity(posts.len());
    let mut tags = BTreeSet::new();
    let mut types = BTreeSet::new();
    let mut years = BTreeSet::new();

    for post in posts {
        let language = canonical_language(&post.language, &language_lookup)
            .unwrap_or_else(|| default_language.clone());

        let mut tag_list = post.tags.clone();
        tag_list.sort();
        tag_list.dedup();
        for tag in &tag_list {
            if !tag.is_empty() {
                tags.insert(tag.clone());
            }
        }

        if let Some(kind) = &post.post_type
            && !kind.trim().is_empty()
        {
            types.insert(kind.clone());
        }

        years.insert(post.date.year());

        let date_iso = post
            .date
            .format(&Rfc3339)
            .context("failed to format post date (rfc3339)")?;
        let date_display = format_date(config, &post.date)?;

        let excerpt = post
            .abstract_text
            .clone()
            .or_else(|| {
                if post.excerpt.trim().is_empty() {
                    None
                } else {
                    Some(post.excerpt.clone())
                }
            })
            .unwrap_or_else(|| post.title.clone().unwrap_or_else(|| post.slug.clone()));

        documents.push(SearchDocument {
            id: post.permalink.clone(),
            title: post.title.clone().unwrap_or_else(|| post.slug.clone()),
            url: post.permalink.clone(),
            language,
            tags: tag_list,
            kind: post.post_type.clone(),
            date_display,
            date_iso,
            timestamp: post.date.unix_timestamp(),
            excerpt,
            content: post.search_text.clone(),
        });
    }

    let languages = config
        .search
        .languages
        .iter()
        .map(|entry| SearchLanguageMeta {
            id: entry.id.clone(),
            name: entry.name.clone(),
            stopwords: normalize_stopwords(&entry.stopwords),
        })
        .collect();

    let index = SearchIndex {
        version: 1,
        generated_at,
        default_language,
        languages,
        documents,
        facets: SearchFacets {
            tags: tags.into_iter().collect(),
            types: types.into_iter().collect(),
            years: years.into_iter().collect(),
        },
    };

    let bytes = serde_json::to_vec(&index).context("failed to serialize search index")?;
    let mut hasher = Hasher::new();
    hasher.update(&bytes);
    let digest = hasher.finalize().to_hex().to_string();

    Ok(SearchIndexArtifact {
        digest,
        bytes,
        document_count: index.documents.len(),
    })
}

pub fn resolve_asset_path(html_root: &Path, asset_path: &str) -> PathBuf {
    let trimmed = asset_path.trim_start_matches('/');
    html_root.join(trimmed)
}

fn normalize_stopwords(stopwords: &[String]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for item in stopwords {
        let word = item.trim().to_lowercase();
        if !word.is_empty() {
            set.insert(word);
        }
    }
    set.into_iter().collect()
}

fn language_lookup(languages: &[SearchLanguageConfig]) -> BTreeMap<String, String> {
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

fn language_aliases(id: &str) -> Vec<String> {
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

fn canonical_language(value: &str, map: &BTreeMap<String, String>) -> Option<String> {
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

fn sanitize_language(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}

fn format_date(config: &Config, date: &OffsetDateTime) -> Result<String> {
    if config.date_format.eq_ignore_ascii_case("RFC3339") {
        return date
            .format(&Rfc3339)
            .context("failed to format RFC3339 date");
    }

    let description = format_description::parse(&config.date_format).with_context(|| {
        format!(
            "invalid date_format '{}' while building search index",
            config.date_format
        )
    })?;
    date.format(&description).with_context(|| {
        format!(
            "failed to format date with pattern '{}' while building search index",
            config.date_format
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Post;
    use serde_json::Value as JsonValue;
    use std::path::PathBuf;

    fn build_post(slug: &str, language: &str, tags: &[&str]) -> Post {
        let date = OffsetDateTime::parse("2024-01-01T12:00:00Z", &Rfc3339).unwrap();
        Post {
            title: Some("Example".to_string()),
            slug: slug.to_string(),
            date,
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            post_type: Some("note".to_string()),
            abstract_text: Some("Summary".to_string()),
            attached: Vec::new(),
            body_html: "<p>Example body</p>".to_string(),
            excerpt: "Example body".to_string(),
            language: language.to_string(),
            search_text: "Example body for search indexing".to_string(),
            source_dir: PathBuf::from("posts/example"),
            content_path: PathBuf::from("posts/example/post.md"),
            permalink: format!("/2024/01/01/{slug}/"),
            extra: serde_json::Map::new(),
        }
    }

    #[test]
    fn build_index_serializes_documents() {
        let config = Config::default();
        let posts = vec![build_post("alpha", "en", &["rust", "notes"])];
        let artifact = build_index(&config, &posts).unwrap();
        let payload: JsonValue = serde_json::from_slice(&artifact.bytes).unwrap();

        let documents = payload["documents"].as_array().unwrap();
        assert_eq!(documents.len(), 1);
        let document = &documents[0];
        assert_eq!(document["language"], JsonValue::String("en".into()));
        assert_eq!(document["tags"].as_array().unwrap().len(), 2);

        let facets = payload["facets"].as_object().unwrap();
        let tags = facets["tags"].as_array().unwrap();
        assert!(tags.iter().any(|value| value == "rust"));
    }

    #[test]
    fn language_aliases_map_to_configured_ids() {
        let config = Config::default();
        let posts = vec![build_post("beta", "eng", &[])];
        let artifact = build_index(&config, &posts).unwrap();
        let payload: JsonValue = serde_json::from_slice(&artifact.bytes).unwrap();
        let document_language = payload["documents"][0]["language"].as_str().unwrap();
        assert_eq!(document_language, "en");
    }
}
