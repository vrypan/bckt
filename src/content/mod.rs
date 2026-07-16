use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use serde_yaml::Mapping;
use time::format_description::{self, well_known::Rfc3339};
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};
use walkdir::WalkDir;

use crate::config::Config;
use crate::language::sanitize_language;
use crate::markdown::{MarkdownRender, render_markdown};
use crate::utils::split_csv;

const MAIN_EXTENSIONS: &[&str] = &["md", "html"];

/// sled key prefix for the parsed-markdown cache. Entries are keyed by each
/// post directory's path relative to the posts root.
const PARSED_BODY_PREFIX: &str = "parsed:";

/// Cached result of rendering a markdown body. Stored per post directory and
/// reused when the body is byte-identical and the binary version is unchanged.
/// The crate version salts the cache so a release (which may change comrak
/// options in `src/markdown.rs`) conservatively invalidates every entry.
#[derive(Serialize, Deserialize)]
struct CachedBody {
    version: String,
    body_hash: String,
    html: String,
    excerpt: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Post {
    pub title: Option<String>,
    pub slug: String,
    pub date: OffsetDateTime,
    pub tags: Vec<String>,
    pub post_type: Option<String>,
    pub abstract_text: Option<String>,
    pub attached: Vec<PathBuf>,
    pub body_html: String,
    pub excerpt: String,
    pub language: String,
    pub search_text: String,
    pub source_dir: PathBuf,
    pub content_path: PathBuf,
    pub content_hash: String,
    pub permalink: String,
    pub extra: JsonMap<String, JsonValue>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FrontMatter {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub date: Option<String>,
    #[serde(deserialize_with = "deserialize_string_or_list")]
    pub tags: Vec<String>,
    #[serde(rename = "type")]
    pub post_type: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub language: Option<String>,
    #[serde(deserialize_with = "deserialize_path_list")]
    pub attached: Vec<PathBuf>,
    #[serde(flatten)]
    pub extra: Mapping,
}

pub fn discover_posts(
    root: impl AsRef<Path>,
    config: &Config,
    cache: Option<&sled::Db>,
) -> Result<Vec<Post>> {
    let root = root.as_ref();
    if !root.exists() {
        bail!("posts directory {} does not exist", root.display());
    }

    let mut posts = Vec::new();
    // Cache keys of the posts seen this render, used to GC stale entries below.
    let mut live_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for entry in WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| {
            // Skip directories that contain .bcktignore file
            if e.file_type().is_dir() {
                !e.path().join(".bcktignore").exists()
            } else {
                true
            }
        })
    {
        let entry = entry?;
        if !entry.file_type().is_dir() {
            continue;
        }
        let cache_key = parsed_body_key(root, entry.path());
        match load_post(entry.path(), config, cache, &cache_key)? {
            Some(post) => {
                live_keys.insert(cache_key);
                posts.push(post);
            }
            None => continue,
        }
    }

    posts.sort_by(|left, right| match left.date.cmp(&right.date) {
        std::cmp::Ordering::Equal => left.slug.cmp(&right.slug),
        other => other,
    });

    let mut seen: std::collections::HashMap<&str, &Path> = std::collections::HashMap::new();
    for post in &posts {
        if let Some(existing) = seen.insert(post.permalink.as_str(), post.source_dir.as_path()) {
            bail!(
                "duplicate permalink {}: defined by both {} and {}",
                post.permalink,
                existing.display(),
                post.source_dir.display()
            );
        }
    }

    if let Some(db) = cache {
        cleanup_parsed_bodies(db, &live_keys)?;
    }

    Ok(posts)
}

/// Build the parsed-markdown cache key for a post directory: the directory path
/// relative to the posts root, joined with `/` for OS-independent determinism.
fn parsed_body_key(root: &Path, dir: &Path) -> String {
    let rel = dir.strip_prefix(root).unwrap_or(dir);
    let joined = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    format!("{PARSED_BODY_PREFIX}{joined}")
}

/// Remove parsed-markdown cache entries for post directories that no longer exist.
fn cleanup_parsed_bodies(db: &sled::Db, keep: &std::collections::BTreeSet<String>) -> Result<()> {
    let mut stale: Vec<Vec<u8>> = Vec::new();
    for entry in db.scan_prefix(PARSED_BODY_PREFIX.as_bytes()) {
        let (key, _) = entry.context("failed to iterate parsed-body cache entries")?;
        let key_str =
            String::from_utf8(key.to_vec()).context("parsed-body cache key is not valid utf-8")?;
        if !keep.contains(&key_str) {
            stale.push(key_str.into_bytes());
        }
    }
    for key in stale {
        db.remove(&key)
            .context("failed to remove stale parsed-body cache entry")?;
    }
    Ok(())
}

fn load_post(
    dir: &Path,
    config: &Config,
    cache: Option<&sled::Db>,
    cache_key: &str,
) -> Result<Option<Post>> {
    let mut main_files = Vec::new();
    for entry in
        fs::read_dir(dir).with_context(|| format!("failed to enumerate {}", dir.display()))?
    {
        let entry = entry?;
        if entry.file_type()?.is_file() && is_main_file(&entry.path()) {
            main_files.push(entry.path());
        }
    }

    if main_files.is_empty() {
        return Ok(None);
    }

    if main_files.len() > 1 {
        bail!(
            "{}: expected exactly one main content file, found {}",
            dir.display(),
            main_files.len()
        );
    }

    let content_path = main_files.remove(0);
    let raw = fs::read_to_string(&content_path)
        .with_context(|| format!("failed to read {}", content_path.display()))?;
    let content_hash = blake3::hash(raw.as_bytes()).to_hex().to_string();
    let (front, body) = parse_front_matter(&raw).with_context(|| {
        format!(
            "{}: missing or invalid front matter",
            content_path.display()
        )
    })?;

    let date_str = front
        .date
        .as_ref()
        .with_context(|| format!("{}: date is required", content_path.display()))?;
    let date = parse_post_date(date_str, config, &content_path)?;

    let slug = determine_slug(dir, front.slug.as_deref())?;
    let permalink = build_permalink(&date, &slug);

    let (body_html, excerpt) = render_body_cached(&content_path, &body, cache, cache_key)?;
    let plain_text = to_plain_text(&body_html);

    let post_type = normalize_post_type(front.post_type.as_deref(), &content_path)?;

    let language = determine_language(front.language.as_deref(), config);

    let extras = mapping_to_json_map(&front.extra).with_context(|| {
        format!(
            "{}: front matter keys must be strings",
            content_path.display()
        )
    })?;

    let post = Post {
        title: front.title,
        slug,
        date,
        tags: front.tags,
        post_type,
        abstract_text: front.abstract_text,
        attached: front.attached,
        body_html,
        excerpt,
        language,
        search_text: plain_text,
        source_dir: dir.to_path_buf(),
        content_path,
        content_hash,
        permalink,
        extra: extras,
    };

    Ok(Some(post))
}

fn normalize_post_type(value: Option<&str>, origin: &Path) -> Result<Option<String>> {
    let Some(raw) = value else {
        return Ok(None);
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let normalized = trimmed.to_ascii_lowercase();
    let valid = normalized
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'));

    if !valid {
        bail!(
            "{}: type may only contain lowercase letters, digits, '-' or '_'",
            origin.display()
        );
    }

    Ok(Some(normalized))
}

fn parse_post_date(date_str: &str, config: &Config, origin: &Path) -> Result<OffsetDateTime> {
    if let Ok(datetime) = OffsetDateTime::parse(date_str, &Rfc3339) {
        return Ok(datetime);
    }

    let naive_format = format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
        .expect("static datetime format to parse");

    if let Ok(datetime) = PrimitiveDateTime::parse(date_str, &naive_format) {
        let offset = config.default_offset().with_context(|| {
            format!(
                "{}: default_timezone '{}' is invalid",
                origin.display(),
                config.default_timezone
            )
        })?;
        return Ok(datetime.assume_offset(offset));
    }

    if let Some((main, offset_part)) = date_str.rsplit_once(' ')
        && let Ok(datetime) = PrimitiveDateTime::parse(main, &naive_format)
        && let Ok(offset) = parse_offset_str(offset_part)
    {
        return Ok(datetime.assume_offset(offset));
    }

    bail!(
        "{}: date must be RFC3339, 'YYYY-MM-DD HH:MM:SS', or 'YYYY-MM-DD HH:MM:SS ±HHMM/±HH:MM'",
        origin.display()
    )
}

fn determine_language(value: Option<&str>, config: &Config) -> String {
    if let Some(explicit) = value {
        let sanitized = sanitize_language(explicit);
        if !sanitized.is_empty() {
            return sanitized;
        }
    }
    sanitize_language(&config.search.default_language)
}

fn to_plain_text(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut last_space = false;

    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                continue;
            }
            '>' => {
                in_tag = false;
                continue;
            }
            _ => {}
        }

        if in_tag {
            continue;
        }

        let normalized = if ch.is_whitespace() { ' ' } else { ch };
        if normalized == ' ' {
            if !last_space {
                result.push(' ');
                last_space = true;
            }
        } else {
            result.push(normalized);
            last_space = false;
        }
    }

    result.trim().to_string()
}

fn parse_offset_str(value: &str) -> Result<UtcOffset> {
    if value.eq_ignore_ascii_case("UTC") || value.eq_ignore_ascii_case("Z") {
        return Ok(UtcOffset::UTC);
    }

    let trimmed = value.trim();
    if trimmed.len() < 3 {
        bail!("offset '{}' is too short", value);
    }

    let normalized = if trimmed.len() == 5
        && trimmed.is_ascii()
        && (trimmed.starts_with('+') || trimmed.starts_with('-'))
    {
        format!("{}:{}", &trimmed[..3], &trimmed[3..])
    } else {
        trimmed.to_string()
    };

    if let Ok(offset) = UtcOffset::parse(
        &normalized,
        &format_description::parse("[offset_hour sign:mandatory]:[offset_minute]")
            .expect("offset format to parse"),
    ) {
        return Ok(offset);
    }

    if let Ok(offset) = UtcOffset::parse(
        &normalized,
        &format_description::parse("[offset_hour sign:mandatory]:[offset_minute]:[offset_second]")
            .expect("offset format to parse"),
    ) {
        return Ok(offset);
    }

    bail!("offset '{}' is invalid", value)
}

fn determine_slug(dir: &Path, provided: Option<&str>) -> Result<String> {
    let raw = if let Some(value) = provided {
        value
    } else {
        dir.file_name()
            .and_then(|value| value.to_str())
            .with_context(|| format!("{}: directory name not valid utf-8", dir.display()))?
    };

    let candidate = crate::utils::slugify(raw);
    if candidate.is_empty() {
        bail!("{}: slug cannot be empty", dir.display());
    }
    Ok(candidate)
}

fn is_main_file(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => {
            let ext = ext.to_ascii_lowercase();
            MAIN_EXTENSIONS.iter().any(|candidate| candidate == &ext)
        }
        None => false,
    }
}

fn parse_front_matter(raw: &str) -> Result<(FrontMatter, String)> {
    let mut lines = raw.lines();
    match lines.next() {
        Some(line) if line.trim() == "---" => {}
        _ => bail!("front matter must start with ---"),
    }

    let mut yaml_lines = Vec::new();
    for line in &mut lines {
        if line.trim() == "---" {
            let yaml = yaml_lines.join("\n");
            let front: FrontMatter = if yaml.trim().is_empty() {
                FrontMatter::default()
            } else {
                serde_yaml::from_str(&yaml)?
            };
            let mut body = lines.collect::<Vec<_>>().join("\n");
            if body.starts_with('\n') {
                body.remove(0);
            }
            return Ok((front, body));
        }
        yaml_lines.push(line);
    }

    bail!("front matter not terminated with ---")
}

fn deserialize_string_or_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Many(Vec<String>),
        One(String),
        None(serde::de::IgnoredAny),
    }

    Ok(match Value::deserialize(deserializer)? {
        Value::Many(items) => items
            .into_iter()
            .map(|item| item.trim().to_string())
            .collect(),
        Value::One(value) => split_csv(&value),
        Value::None(_) => Vec::new(),
    })
}

fn deserialize_path_list<'de, D>(deserializer: D) -> Result<Vec<PathBuf>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Many(Vec<PathBuf>),
        One(String),
        None(serde::de::IgnoredAny),
    }

    Ok(match Value::deserialize(deserializer)? {
        Value::Many(items) => items,
        Value::One(value) => split_csv(&value).into_iter().map(PathBuf::from).collect(),
        Value::None(_) => Vec::new(),
    })
}

fn mapping_to_json_map(mapping: &Mapping) -> Result<JsonMap<String, JsonValue>> {
    let mut map = JsonMap::new();
    for (key, value) in mapping {
        let key = key
            .as_str()
            .with_context(|| format!("front matter key {key:?} is not a string"))?;
        let json = serde_json::to_value(value)
            .with_context(|| format!("failed to convert front matter value for '{key}'"))?;
        map.insert(key.to_string(), json);
    }
    Ok(map)
}

fn build_permalink(date: &OffsetDateTime, slug: &str) -> String {
    format!(
        "/{:04}/{:02}/{:02}/{slug}/",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// Render a post body, reusing a cached markdown render when the body is
/// byte-identical to a previous render (same binary version). Only markdown is
/// cached; HTML bodies are cheap (trim + excerpt) and always rendered inline.
fn render_body_cached(
    path: &Path,
    body: &str,
    cache: Option<&sled::Db>,
    cache_key: &str,
) -> Result<(String, String)> {
    let is_md = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
    if !is_md {
        return render_body(path, body);
    }

    let body_hash = blake3::hash(body.as_bytes()).to_hex().to_string();
    let version = env!("CARGO_PKG_VERSION");

    // Cache hit: a corrupt/undeserializable entry is treated as a miss.
    if let Some(db) = cache
        && let Ok(Some(bytes)) = db.get(cache_key.as_bytes())
        && let Ok(cached) = serde_json::from_slice::<CachedBody>(&bytes)
        && cached.version == version
        && cached.body_hash == body_hash
    {
        return Ok((cached.html, cached.excerpt));
    }

    let (html, excerpt) = render_body(path, body)?;

    if let Some(db) = cache {
        let record = CachedBody {
            version: version.to_string(),
            body_hash,
            html: html.clone(),
            excerpt: excerpt.clone(),
        };
        // Cache-write failures are non-fatal; worst case is a re-render next time.
        if let Ok(bytes) = serde_json::to_vec(&record) {
            let _ = db.insert(cache_key.as_bytes(), bytes);
        }
    }

    Ok((html, excerpt))
}

fn render_body(path: &Path, body: &str) -> Result<(String, String)> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("md") => {
            let MarkdownRender { html, excerpt } = render_markdown(body);
            Ok((html, excerpt))
        }
        Some(ext) if ext.eq_ignore_ascii_case("html") => {
            let clean = body.trim().to_string();
            let excerpt = excerpt_from_html(&clean);
            Ok((clean, excerpt))
        }
        _ => bail!("{}: unsupported content extension", path.display()),
    }
}

fn excerpt_from_html(html: &str) -> String {
    const LIMIT: usize = 280;
    let mut plain = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                plain.push(' ');
            }
            _ if !in_tag => plain.push(ch),
            _ => {}
        }
    }
    let text = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return String::new();
    }
    let mut excerpt = String::new();
    let mut count = 0;
    let total = text.chars().count();
    for ch in text.chars() {
        if count >= LIMIT {
            break;
        }
        excerpt.push(ch);
        count += 1;
    }
    if total > count {
        excerpt.push_str("...");
    }
    excerpt.trim().to_string()
}

#[cfg(test)]
mod tests;
