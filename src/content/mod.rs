use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use serde_yaml::Mapping;
use time::format_description::{self, well_known::Rfc3339};
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};
use walkdir::WalkDir;

use crate::config::Config;
use crate::markdown::{MarkdownRender, render_markdown};
use isolang::Language;
use whatlang::detect;

const MAIN_EXTENSIONS: &[&str] = &["md", "html"];

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

pub fn discover_posts(root: impl AsRef<Path>, config: &Config) -> Result<Vec<Post>> {
    let root = root.as_ref();
    if !root.exists() {
        bail!("posts directory {} does not exist", root.display());
    }

    let mut posts = Vec::new();

    for entry in WalkDir::new(root).min_depth(1) {
        let entry = entry?;
        if !entry.file_type().is_dir() {
            continue;
        }
        match load_post(entry.path(), config)? {
            Some(post) => posts.push(post),
            None => continue,
        }
    }

    posts.sort_by(|left, right| match left.date.cmp(&right.date) {
        std::cmp::Ordering::Equal => left.slug.cmp(&right.slug),
        other => other,
    });
    Ok(posts)
}

fn load_post(dir: &Path, config: &Config) -> Result<Option<Post>> {
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

    let (body_html, excerpt) = render_body(&content_path, &body)?;
    let plain_text = to_plain_text(&body_html);

    let post_type = normalize_post_type(front.post_type.as_deref(), &content_path)?;

    let language = determine_language(front.language.as_deref(), &plain_text, config);

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

fn determine_language(value: Option<&str>, body_text: &str, config: &Config) -> String {
    let languages = language_lookup(config);

    if let Some(explicit) = value
        && let Some(tag) = canonical_language(explicit, &languages)
    {
        return tag;
    }

    if let Some(guessed) = guess_language(body_text)
        && let Some(tag) = canonical_language(&guessed, &languages)
    {
        return tag;
    }

    canonical_language(&config.search.default_language, &languages)
        .unwrap_or_else(|| sanitize_language(&config.search.default_language))
}

fn language_lookup(config: &Config) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for entry in &config.search.languages {
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

fn guess_language(body_text: &str) -> Option<String> {
    let trimmed = body_text.trim();
    if trimmed.chars().count() < 24 {
        return None;
    }

    let info = detect(trimmed)?;
    if !info.is_reliable() {
        return None;
    }

    let iso3 = info.lang().code();
    if let Some(lang) = Language::from_639_3(iso3) {
        if let Some(code) = lang.to_639_1() {
            return Some(code.to_lowercase());
        }
        return Some(lang.to_639_3().to_lowercase());
    }

    Some(iso3.to_lowercase())
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

    let normalized = if trimmed.len() == 5 && (trimmed.starts_with('+') || trimmed.starts_with('-'))
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

    let candidate = slugify(raw);
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

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    slug
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
        Value::One(value) => split_csv(&value)
            .into_iter()
            .map(|item| item.to_string())
            .collect(),
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

fn split_csv(input: &str) -> Vec<&str> {
    input
        .split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect()
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
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use time::UtcOffset;

    #[test]
    fn discover_single_markdown_post() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts");
        fs::create_dir_all(root.join("notes/hello-world")).unwrap();
        fs::write(
            root.join("notes/hello-world/post.md"),
            "---\ntitle: Hello\ndate: 2024-02-01T12:00:00Z\ntags: [rust]\n---\nBody",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(&root, &config).unwrap();
        assert_eq!(posts.len(), 1);
        let post = &posts[0];
        assert_eq!(post.slug, "hello-world");
        assert_eq!(post.tags, vec!["rust".to_string()]);
        assert_eq!(post.permalink, "/2024/02/01/hello-world/");
        assert_eq!(post.body_html, "<p>Body</p>\n");
        assert_eq!(post.excerpt, "Body");
    }

    #[test]
    fn prefer_slug_from_front_matter() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts");
        fs::create_dir_all(root.join("mixed/Example")).unwrap();
        fs::write(
            root.join("mixed/Example/post.md"),
            "---\ndate: 2024-03-04T00:00:00Z\nslug: Custom Slug\n---\n",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(&root, &config).unwrap();
        assert_eq!(posts[0].slug, "custom-slug");
    }

    #[test]
    fn parse_full_front_matter_payload() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/full");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ntitle: Sample\ndate: 2024-05-06T08:09:10Z\ntags:\n  - summary\n  - rust\nabstract: Short\nattached:\n  - files/data.csv\nimages:\n  - img.png\nvideo_url: https://example.com/video.mp4\nlocation:\n  country: GR\n---\nBody\n",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        let post = &posts[0];
        assert_eq!(post.title.as_deref(), Some("Sample"));
        assert_eq!(post.tags, vec!["summary".to_string(), "rust".to_string()]);
        assert_eq!(post.abstract_text.as_deref(), Some("Short"));
        assert_eq!(post.attached, vec![PathBuf::from("files/data.csv")]);
        assert_eq!(post.body_html, "<p>Body</p>\n");
        assert_eq!(post.excerpt, "Body");
        assert_eq!(
            post.extra
                .get("location")
                .and_then(|value| value.get("country")),
            Some(&JsonValue::String("GR".to_string()))
        );
        assert_eq!(
            post.extra.get("images"),
            Some(&JsonValue::Array(vec![JsonValue::String("img.png".into())]))
        );
        assert_eq!(
            post.extra.get("video_url"),
            Some(&JsonValue::String("https://example.com/video.mp4".into()))
        );
    }

    #[test]
    fn reject_duplicate_main_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/dupe");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.md"), "---\ndate: 2024-01-01T00:00:00Z\n---\n").unwrap();
        fs::write(
            root.join("b.html"),
            "---\ndate: 2024-01-01T00:00:00Z\n---\n",
        )
        .unwrap();

        let config = Config::default();
        let error = discover_posts(root.parent().unwrap(), &config).unwrap_err();
        assert!(format!("{error}").contains("expected exactly one"));
    }

    #[test]
    fn reject_missing_front_matter() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/missing");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("post.md"), "no front matter").unwrap();

        let config = Config::default();
        let error = discover_posts(root.parent().unwrap(), &config).unwrap_err();
        assert!(format!("{error}").contains("front matter"));
    }

    #[test]
    fn allow_front_matter_only() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/solo");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\n---\n",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        assert_eq!(posts[0].body_html, "");
        assert_eq!(posts[0].excerpt, "");
    }

    #[test]
    fn retains_additional_front_matter() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/extras");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\nlocation:\n  country: GR\n  city: Athens\n---\n",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        let value = posts[0]
            .extra
            .get("location")
            .and_then(|map| map.get("city"))
            .cloned();

        assert_eq!(value, Some(JsonValue::String("Athens".to_string())));
    }

    #[test]
    fn parse_comma_separated_lists() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/list");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\ntags: one, two , three\nattached: file-a.txt, file-b.txt\nimages: img-a.png\n---\n",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        let post = &posts[0];

        assert_eq!(post.tags, vec!["one", "two", "three"]);
        assert_eq!(
            post.attached,
            vec![PathBuf::from("file-a.txt"), PathBuf::from("file-b.txt")]
        );
        assert_eq!(
            post.extra.get("images"),
            Some(&JsonValue::String("img-a.png".into()))
        );
    }

    #[test]
    fn allows_empty_tags_field() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/empty-tags");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\ntags:\n---\nBody",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        assert!(posts[0].tags.is_empty());
    }

    #[test]
    fn allows_empty_attached_field() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/empty-attached");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\nattached:\n---\nBody",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        assert!(posts[0].attached.is_empty());
    }

    #[test]
    fn accepts_datetime_with_numeric_offset() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/offset");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ndate: 2013-01-18 00:25:24 +0200\n---\nBody",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        let post = &posts[0];
        assert_eq!(post.date.offset(), UtcOffset::from_hms(2, 0, 0).unwrap());
    }

    #[test]
    fn accepts_naive_datetime_with_default_timezone() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/naive");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ndate: 2024-01-02 09:30:00\n---\nBody",
        )
        .unwrap();

        let config = Config {
            default_timezone: "+02:00".to_string(),
            ..Default::default()
        };

        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        let post = &posts[0];
        let offset = config.default_offset().unwrap();
        assert_eq!(post.date.offset(), offset);
        assert_eq!(post.date.hour(), 9);
        assert_eq!(post.date.minute(), 30);
        assert_eq!(post.excerpt, "Body");
    }

    #[test]
    fn language_from_front_matter_is_normalized() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/lang");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\nlanguage: EL\n---\nΔοκιμαστικό κείμενο.",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        assert_eq!(posts[0].language, "el");
    }

    #[test]
    fn language_is_detected_when_missing() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/detect");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\n---\nΑυτό είναι ένα παράδειγμα ελληνικού κειμένου για την ανίχνευση γλώσσας.",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        assert_eq!(posts[0].language, "el");
    }

    #[test]
    fn short_content_falls_back_to_default_language() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/fallback");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\n---\nHi!",
        )
        .unwrap();

        let mut config = Config::default();
        config.search.default_language = "en".to_string();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        assert_eq!(posts[0].language, "en");
    }

    #[test]
    fn slugify_directory_name() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("  Multi   Spaces  "), "multi-spaces");
    }

    #[test]
    fn html_posts_are_passthrough() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("posts/page");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("post.html"),
            "---\ndate: 2024-01-02T00:00:00Z\n---\n<p>Sunny</p>",
        )
        .unwrap();

        let config = Config::default();
        let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
        assert_eq!(posts[0].body_html, "<p>Sunny</p>");
        assert_eq!(posts[0].excerpt, "Sunny");
    }
}
