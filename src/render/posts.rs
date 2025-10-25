use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use blake3::Hasher;
use minijinja::Environment;
use serde::Serialize;
use serde_json::Value as JsonValue;
use time::OffsetDateTime;
use time::format_description;

use crate::config::Config;
use crate::content::{Post, discover_posts};
use crate::utils::absolute_url;

use super::templates::render_template_with_scope;
use super::utils::{log_status, normalize_path};
use super::{BuildMode, POST_HASH_PREFIX};

pub(super) fn render_posts(
    root: &Path,
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache_db: &sled::Db,
    mode: BuildMode,
    verbose: bool,
) -> Result<(Vec<Post>, usize, usize)> {
    let posts_dir = root.join("posts");
    let mut posts = discover_posts(&posts_dir, config)?;
    if posts.is_empty() {
        return Ok((posts, 0, 0));
    }

    posts.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.slug.cmp(&b.slug)));

    let default_post_template = env
        .get_template("post.html")
        .context("post.html template missing")?;

    let mut cache_keys: BTreeSet<String> = BTreeSet::new();

    let mut rendered_count = 0usize;
    let mut skipped_count = 0usize;

    for post in &posts {
        let cache_key = format!("{POST_HASH_PREFIX}{}", post.permalink);
        cache_keys.insert(cache_key.clone());

        let digest = compute_post_digest(post)?;
        let cached = cache_db
            .get(cache_key.as_bytes())
            .with_context(|| format!("failed to read cache entry for {}", post.slug))?;
        let digest_bytes = digest.as_bytes();
        let needs_render = if matches!(mode, BuildMode::Full) {
            true
        } else if let Some(value) = cached.as_ref() {
            value.as_ref() != digest_bytes
        } else {
            true
        };

        if needs_render {
            rendered_count += 1;
            let render_target = html_root.join(post.permalink.trim_start_matches('/'));
            let output_path = render_target.join("index.html");
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }

            let context = build_post_context(config, post)?;
            let template_name = post
                .post_type
                .as_deref()
                .map(|value| format!("post-{value}.html"))
                .unwrap_or_else(|| "post.html".to_string());

            let scope = format!("rendering post {}", post.slug);
            let rendered = if template_name == "post.html" {
                render_template_with_scope(
                    &default_post_template,
                    minijinja::context! { post => &context },
                    &scope,
                )
            } else {
                match env.get_template(&template_name) {
                    Ok(tpl) => render_template_with_scope(
                        &tpl,
                        minijinja::context! { post => &context },
                        &scope,
                    ),
                    Err(err) => {
                        log_status(
                            verbose,
                            "WARN",
                            format!(
                                "{}: missing {} ({}); using post.html",
                                post.slug, template_name, err
                            ),
                        );
                        render_template_with_scope(
                            &default_post_template,
                            minijinja::context! { post => &context },
                            &scope,
                        )
                    }
                }
            }?;

            fs::write(&output_path, rendered)
                .with_context(|| format!("failed to write {}", output_path.display()))?;

            copy_post_assets(post, &render_target)
                .with_context(|| format!("failed to copy assets for {}", post.slug))?;

            log_status(
                verbose,
                "RENDER",
                format!("Rendered post {}", post.permalink),
            );
        } else {
            skipped_count += 1;
            log_status(
                verbose,
                "SKIP",
                format!("Post {} unchanged", post.permalink),
            );
        }

        cache_db
            .insert(cache_key.as_bytes(), digest_bytes)
            .with_context(|| format!("failed to update cache entry for {}", post.slug))?;
    }

    cleanup_post_hashes(cache_db, &cache_keys)?;

    Ok((posts, rendered_count, skipped_count))
}

pub(super) fn post_key(post: &Post) -> String {
    format!("{}-{}", post.date.unix_timestamp(), post.slug)
}

fn build_post_context(config: &Config, post: &Post) -> Result<PostTemplate> {
    let date = format_date(config, &post.date)?;
    let date_iso = post
        .date
        .format(&time::format_description::well_known::Rfc3339)
        .context("failed to format RFC3339 date")?;

    let attached = convert_paths(&post.attached)?;
    let body = att_to_absolute(
        &post.body_html,
        &post.permalink,
        &config.base_url,
        &post.attached,
        false,
    );

    // Build attachments metadata map
    let mut attachments = HashMap::new();
    for relative_path in &post.attached {
        let normalized = normalize_path(relative_path);
        let asset_path = post.source_dir.join(relative_path);

        if let Ok(metadata) = fs::metadata(&asset_path) {
            let size = metadata.len();
            let mime_type = mime_guess::from_path(&asset_path)
                .first_or_octet_stream()
                .to_string();

            attachments.insert(normalized, AttachmentMeta { size, mime_type });
        }
    }

    Ok(PostTemplate {
        title: post.title.clone(),
        slug: post.slug.clone(),
        date,
        date_iso,
        language: post.language.clone(),
        tags: post.tags.clone(),
        post_type: post.post_type.clone(),
        abstract_text: post.abstract_text.clone(),
        attached,
        body,
        excerpt: post.excerpt.clone(),
        permalink: post.permalink.clone(),
        attachments,
        extra: post.extra.clone(),
    })
}

pub(super) fn build_post_summary(config: &Config, post: &Post) -> Result<PostSummary> {
    let date = format_date(config, &post.date)?;
    let date_iso = post
        .date
        .format(&time::format_description::well_known::Rfc3339)
        .context("failed to format RFC3339 date")?;

    let body = att_to_absolute(
        &post.body_html,
        &post.permalink,
        &config.base_url,
        &post.attached,
        false,
    );

    // Build attachments metadata map
    let mut attachments = HashMap::new();
    for relative_path in &post.attached {
        let normalized = normalize_path(relative_path);
        let asset_path = post.source_dir.join(relative_path);

        if let Ok(metadata) = fs::metadata(&asset_path) {
            let size = metadata.len();
            let mime_type = mime_guess::from_path(&asset_path)
                .first_or_octet_stream()
                .to_string();

            attachments.insert(normalized, AttachmentMeta { size, mime_type });
        }
    }

    Ok(PostSummary {
        title: post.title.clone(),
        slug: post.slug.clone(),
        date,
        date_iso,
        language: post.language.clone(),
        tags: post.tags.clone(),
        post_type: post.post_type.clone(),
        abstract_text: post.abstract_text.clone(),
        body,
        excerpt: post.excerpt.clone(),
        permalink: post.permalink.clone(),
        attachments,
        extra: post.extra.clone(),
    })
}

#[derive(Serialize)]
pub(super) struct PostTemplate {
    pub(super) title: Option<String>,
    pub(super) slug: String,
    pub(super) date: String,
    pub(super) date_iso: String,
    pub(super) language: String,
    pub(super) tags: Vec<String>,
    #[serde(rename = "type")]
    pub(super) post_type: Option<String>,
    #[serde(rename = "abstract")]
    pub(super) abstract_text: Option<String>,
    pub(super) attached: Vec<String>,
    pub(super) body: String,
    pub(super) excerpt: String,
    pub(super) permalink: String,
    pub(super) attachments: HashMap<String, AttachmentMeta>,
    #[serde(flatten)]
    pub(super) extra: serde_json::Map<String, JsonValue>,
}

#[derive(Serialize)]
pub(super) struct AttachmentMeta {
    pub(super) size: u64,
    pub(super) mime_type: String,
}

#[derive(Serialize)]
pub(super) struct PostSummary {
    pub(super) title: Option<String>,
    pub(super) slug: String,
    pub(super) date: String,
    pub(super) date_iso: String,
    pub(super) language: String,
    pub(super) tags: Vec<String>,
    #[serde(rename = "type")]
    pub(super) post_type: Option<String>,
    #[serde(rename = "abstract")]
    pub(super) abstract_text: Option<String>,
    pub(super) body: String,
    pub(super) excerpt: String,
    pub(super) permalink: String,
    pub(super) attachments: HashMap<String, AttachmentMeta>,
    #[serde(flatten)]
    pub(super) extra: serde_json::Map<String, JsonValue>,
}

fn compute_post_digest(post: &Post) -> Result<String> {
    let mut hasher = Hasher::new();
    let content = fs::read(&post.content_path).with_context(|| {
        format!(
            "failed to read content file {}",
            post.content_path.display()
        )
    })?;
    hasher.update(&content);

    let mut assets: Vec<PathBuf> = post.attached.clone();
    assets.sort();

    for relative in assets {
        let normalized = normalize_path(&relative);
        hasher.update(normalized.as_bytes());
        let asset_path = post.source_dir.join(&relative);
        let metadata = fs::metadata(&asset_path)
            .with_context(|| format!("failed to inspect asset {}", asset_path.display()))?;
        hasher.update(&metadata.len().to_le_bytes());
        let modified = metadata.modified().with_context(|| {
            format!(
                "failed to read modification time for {}",
                asset_path.display()
            )
        })?;
        let duration = modified
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::new(0, 0));
        hasher.update(&duration.as_secs().to_le_bytes());
        hasher.update(&duration.subsec_nanos().to_le_bytes());
    }

    Ok(hasher.finalize().to_hex().to_string())
}

fn cleanup_post_hashes(db: &sled::Db, keep: &BTreeSet<String>) -> Result<()> {
    let mut stale: Vec<Vec<u8>> = Vec::new();
    for entry in db.scan_prefix(POST_HASH_PREFIX.as_bytes()) {
        let (key, _) = entry.context("failed to iterate post cache entries")?;
        let key_vec = key.to_vec();
        let key_str =
            String::from_utf8(key_vec.clone()).context("post cache key is not valid utf-8")?;
        if !keep.contains(&key_str) {
            stale.push(key_vec);
        }
    }

    for key in stale {
        db.remove(&key)
            .context("failed to remove stale post cache entry")?;
    }
    Ok(())
}

fn copy_post_assets(post: &Post, target_dir: &Path) -> Result<()> {
    let mut assets = BTreeSet::new();
    for entry in &post.attached {
        if entry.is_absolute() {
            bail!("{}: asset path must be relative", entry.display());
        }
        assets.insert(entry.clone());
    }

    for relative in assets {
        let source = post.source_dir.join(&relative);
        if !source.exists() {
            bail!("missing asset {}", source.display());
        }
        let destination = target_dir.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(&source, &destination).with_context(|| {
            format!(
                "failed to copy asset from {} to {}",
                source.display(),
                destination.display()
            )
        })?;
    }

    Ok(())
}

fn format_date(config: &Config, date: &OffsetDateTime) -> Result<String> {
    if config.date_format.eq_ignore_ascii_case("RFC3339") {
        return date
            .format(&time::format_description::well_known::Rfc3339)
            .context("failed to format RFC3339 date");
    }

    let description = format_description::parse(&config.date_format)
        .with_context(|| format!("invalid date_format '{}'", config.date_format))?;
    date.format(&description).with_context(|| {
        format!(
            "failed to format date with pattern '{}'",
            config.date_format
        )
    })
}

fn convert_paths(paths: &[PathBuf]) -> Result<Vec<String>> {
    let mut set = BTreeSet::new();
    for path in paths {
        if path.is_absolute() {
            bail!("asset paths must be relative: {}", path.display());
        }
        set.insert(normalize_path(path));
    }
    Ok(set.into_iter().collect())
}

pub(super) fn att_to_absolute(
    body: &str,
    permalink: &str,
    base_url: &str,
    attached: &[PathBuf],
    return_absolute: bool,
) -> String {
    if attached.is_empty() {
        return body.to_string();
    }

    let mut attached_paths: HashSet<String> = HashSet::new();
    for item in attached {
        if item.is_absolute() {
            continue;
        }
        attached_paths.insert(normalize_path(item));
    }
    if attached_paths.is_empty() {
        return body.to_string();
    }

    let mut output = String::with_capacity(body.len());
    let mut i = 0;
    let bytes = body.as_bytes();

    while i < bytes.len() {
        if let Some((quote, prefix_len)) = match_attribute(&body[i..]) {
            output.push_str(&body[i..i + prefix_len]);
            let mut value_end = i + prefix_len;
            while value_end < bytes.len() {
                let ch = body[value_end..].chars().next().unwrap();
                if ch == quote {
                    break;
                }
                value_end += ch.len_utf8();
            }

            if value_end >= bytes.len() {
                output.push_str(&body[i + prefix_len..]);
                break;
            }

            let value = &body[i + prefix_len..value_end];
            if let Some(rewritten) =
                rewrite_if_attached(value, permalink, base_url, &attached_paths, return_absolute)
            {
                output.push_str(&rewritten);
            } else {
                output.push_str(value);
            }

            output.push(quote);
            i = value_end + quote.len_utf8();
        } else {
            let ch = body[i..].chars().next().unwrap();
            output.push(ch);
            i += ch.len_utf8();
        }
    }

    output
}

fn match_attribute(input: &str) -> Option<(char, usize)> {
    if input.starts_with("src=\"") {
        Some(('"', 5))
    } else if input.starts_with("src='") {
        Some(('\'', 5))
    } else if input.starts_with("href=\"") {
        Some(('"', 6))
    } else if input.starts_with("href='") {
        Some(('\'', 6))
    } else {
        None
    }
}

fn rewrite_if_attached(
    value: &str,
    permalink: &str,
    base_url: &str,
    attached: &HashSet<String>,
    return_absolute: bool,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if trimmed.starts_with('/')
        || trimmed.starts_with('#')
        || trimmed.starts_with("//")
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || lower.starts_with("data:")
        || lower.starts_with("javascript:")
    {
        return None;
    }

    let mut relative = trimmed;
    while let Some(stripped) = relative.strip_prefix("./") {
        relative = stripped;
    }
    if relative.is_empty() {
        return None;
    }

    let (path_part, suffix) = match relative.find(['?', '#']) {
        Some(idx) => relative.split_at(idx),
        None => (relative, ""),
    };

    if !attached.contains(path_part) {
        return None;
    }

    if return_absolute {
        let base = join_permalink(permalink, path_part);
        let joined = if suffix.is_empty() {
            base
        } else {
            format!("{}{}", base, suffix)
        };
        Some(absolute_url(base_url, &joined))
    } else {
        // Keep as relative path for HTML rendering - this works regardless of base_url
        // because the file structure matches the URL structure
        if suffix.is_empty() {
            Some(path_part.to_string())
        } else {
            Some(format!("{}{}", path_part, suffix))
        }
    }
}

fn join_permalink(permalink: &str, relative: &str) -> String {
    if relative.starts_with('/') {
        return relative.to_string();
    }

    let mut full = PathBuf::new();
    for segment in permalink.trim_matches('/').split('/') {
        if !segment.is_empty() {
            full.push(segment);
        }
    }
    let trimmed = relative.trim_start_matches("./");
    full.push(trimmed);

    let normalized = normalize_path(full.as_path());
    format!("/{}", normalized)
}
