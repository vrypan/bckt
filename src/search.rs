use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use blake3::Hasher;
use serde::Serialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use time::OffsetDateTime;
use time::format_description;
use time::format_description::well_known::Rfc3339;

use crate::config::Config;
use crate::content::Post;
use crate::language::sanitize_language;

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
struct SearchIndexDigestView<'a> {
    version: u8,
    default_language: &'a str,
    languages: &'a [SearchLanguageMeta],
    documents: &'a [SearchDocument],
    facets: &'a SearchFacets,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<JsonMap<String, JsonValue>>,
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

    let default_language = sanitize_language(&config.search.default_language);

    let mut documents = Vec::with_capacity(posts.len());
    let mut tags = BTreeSet::new();
    let mut types = BTreeSet::new();
    let mut years = BTreeSet::new();

    // Cache date format parsing to avoid repeated parsing
    let date_format = if config.date_format.eq_ignore_ascii_case("RFC3339") {
        None
    } else {
        Some(
            format_description::parse(&config.date_format).with_context(|| {
                format!(
                    "invalid date_format '{}' while building search index",
                    config.date_format
                )
            })?,
        )
    };

    for post in posts {
        let language = if post.language.is_empty() {
            default_language.clone()
        } else {
            post.language.clone()
        };

        let mut seen_tags = std::collections::HashSet::new();
        let mut tag_list = Vec::with_capacity(post.tags.len());
        for tag in &post.tags {
            if !tag.is_empty() && seen_tags.insert(tag.as_str()) {
                tag_list.push(tag.clone());
                tags.insert(tag.clone());
            }
        }
        tag_list.sort_unstable();

        if let Some(kind) = &post.post_type {
            let trimmed = kind.trim();
            if !trimmed.is_empty() {
                types.insert(kind.clone());
            }
        }

        years.insert(post.date.year());

        let date_iso = post
            .date
            .format(&Rfc3339)
            .context("failed to format post date (rfc3339)")?;

        let date_display = match &date_format {
            None => date_iso.clone(),
            Some(format) => post.date.format(format).with_context(|| {
                format!(
                    "failed to format date with pattern '{}' while building search index",
                    config.date_format
                )
            })?,
        };

        // More efficient excerpt selection
        let excerpt = post
            .abstract_text
            .as_ref()
            .or_else(|| {
                let trimmed = post.excerpt.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(&post.excerpt)
                }
            })
            .cloned()
            .unwrap_or_else(|| post.title.as_ref().unwrap_or(&post.slug).clone());

        let title = post.title.as_ref().unwrap_or(&post.slug).clone();

        let mut payload_map = JsonMap::new();
        if !config.search.payload_fields.is_empty() {
            for key in &config.search.payload_fields {
                if let Some(value) = post.extra.get(key)
                    && !value.is_null()
                {
                    payload_map.insert(key.clone(), value.clone());
                }
            }
        }

        documents.push(SearchDocument {
            id: post.permalink.clone(),
            title,
            url: post.permalink.clone(),
            language,
            tags: tag_list,
            kind: post.post_type.clone(),
            date_display,
            date_iso,
            timestamp: post.date.unix_timestamp(),
            excerpt,
            content: post.search_text.clone(),
            payload: if payload_map.is_empty() {
                None
            } else {
                Some(payload_map)
            },
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

    let digest_view = SearchIndexDigestView {
        version: index.version,
        default_language: &index.default_language,
        languages: &index.languages,
        documents: &index.documents,
        facets: &index.facets,
    };
    let digest_input =
        serde_json::to_vec(&digest_view).context("failed to serialize search digest view")?;
    let mut hasher = Hasher::new();
    hasher.update(&digest_input);
    let digest = hasher.finalize().to_hex().to_string();

    let bytes = serde_json::to_vec(&index).context("failed to serialize search index")?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Post;
    use serde_json::{Value as JsonValue, json};
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
    fn digest_is_stable_across_rebuilds_of_same_content() {
        let config = Config::default();
        let posts = vec![build_post("alpha", "en", &["rust"])];
        let first = build_index(&config, &posts).unwrap();
        let second = build_index(&config, &posts).unwrap();
        assert_eq!(first.digest, second.digest);
        // generated_at still varies, so raw bytes may differ — only the digest is stable
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
    fn payload_fields_are_emitted() {
        let mut config = Config::default();
        config.search.payload_fields = vec!["image".into(), "duration".into()];
        let mut post = build_post("gamma", "en", &[]);
        post.extra
            .insert("image".into(), json!("/static/img/cover.jpg"));
        post.extra.insert("duration".into(), json!(128));
        post.extra.insert("ignored".into(), json!("value"));
        let artifact = build_index(&config, &[post]).unwrap();
        let root: JsonValue = serde_json::from_slice(&artifact.bytes).unwrap();
        let payload = root["documents"][0]["payload"].as_object().unwrap();
        assert_eq!(
            payload.get("image").unwrap(),
            &json!("/static/img/cover.jpg")
        );
        assert_eq!(payload.get("duration").unwrap(), &json!(128));
        assert!(payload.get("ignored").is_none());
    }

    #[test]
    fn namespaced_payload_is_ignored() {
        let mut config = Config::default();
        config.search.payload_fields = vec!["image".into(), "duration".into()];
        let mut post = build_post("delta", "en", &[]);
        post.extra.insert(
            "search".into(),
            json!({
                "payload": {
                    "image": "/covers/delta.png",
                    "duration": 42
                },
                "image": "/covers/other.png"
            }),
        );

        let artifact = build_index(&config, &[post]).unwrap();
        let root: JsonValue = serde_json::from_slice(&artifact.bytes).unwrap();
        assert!(root["documents"][0]["payload"].is_null());
    }
}
