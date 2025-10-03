use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use blake3::Hasher;
use minijinja::{Environment, context};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use time::OffsetDateTime;
use time::format_description;
use time::format_description::well_known::{Rfc2822, Rfc3339};
use walkdir::WalkDir;

use crate::config::Config;
use crate::content::{Post, discover_posts};
use crate::template;
use crate::utils::absolute_url;

#[derive(Clone, Copy, Debug)]
pub struct RenderPlan {
    pub posts: bool,
    pub static_assets: bool,
    pub mode: BuildMode,
    pub verbose: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildMode {
    Full,
    Changed,
}

const CACHE_DIR: &str = ".bucket3/cache";
const HOME_PAGES_KEY: &str = "home_pages";
const TAG_PAGES_KEY: &str = "tag_pages";
const POST_HASH_PREFIX: &str = "post:";
const SITE_INPUTS_KEY: &str = "site_inputs_hash";
const STATIC_HASH_KEY: &str = "static_hash";

fn log_status(enabled: bool, label: &str, message: impl AsRef<str>) {
    if enabled {
        println!("[{}] {}", label, message.as_ref());
    }
}

fn config_tag_feeds(config: &Config) -> Vec<String> {
    fn split_list(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    }

    let mut tags = Vec::new();
    if let Some(value) = config.extra.get("rss_tags") {
        match value {
            JsonValue::String(s) => tags.extend(split_list(s)),
            JsonValue::Array(items) => {
                for item in items {
                    if let JsonValue::String(s) = item {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            tags.push(trimmed.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredPage {
    cursor: String,
    posts: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredTagPage {
    tag: String,
    slug: String,
    cursor: String,
    posts: Vec<String>,
}

struct TagBucket {
    name: String,
    slug: String,
    indices: Vec<usize>,
}

struct HomePageCache {
    db: sled::Db,
}

impl HomePageCache {
    fn new(db: sled::Db) -> Self {
        Self { db }
    }

    fn load_pages(&self) -> Result<Vec<StoredPage>> {
        let maybe = self
            .db
            .get(HOME_PAGES_KEY)
            .context("failed to read homepage cache")?;
        if let Some(bytes) = maybe {
            let pages: Vec<StoredPage> =
                serde_json::from_slice(&bytes).context("failed to deserialize homepage cache")?;
            Ok(pages)
        } else {
            Ok(Vec::new())
        }
    }

    fn store_pages(&self, pages: &[StoredPage]) -> Result<()> {
        let data = serde_json::to_vec(pages).context("failed to serialize homepage cache")?;
        self.db
            .insert(HOME_PAGES_KEY, data)
            .context("failed to update homepage cache")?;
        self.db.flush().context("failed to flush homepage cache")?;
        Ok(())
    }
}

struct TagPageCache {
    db: sled::Db,
}

impl TagPageCache {
    fn new(db: sled::Db) -> Self {
        Self { db }
    }

    fn load_pages(&self) -> Result<Vec<StoredTagPage>> {
        let maybe = self
            .db
            .get(TAG_PAGES_KEY)
            .context("failed to read tag cache")?;
        if let Some(bytes) = maybe {
            let pages: Vec<StoredTagPage> =
                serde_json::from_slice(&bytes).context("failed to deserialize tag cache")?;
            Ok(pages)
        } else {
            Ok(Vec::new())
        }
    }

    fn store_pages(&self, pages: &[StoredTagPage]) -> Result<()> {
        let data = serde_json::to_vec(pages).context("failed to serialize tag cache")?;
        self.db
            .insert(TAG_PAGES_KEY, data)
            .context("failed to update tag cache")?;
        self.db.flush().context("failed to flush tag cache")?;
        Ok(())
    }
}

fn open_cache_db(root: &Path) -> Result<sled::Db> {
    let cache_dir = root.join(CACHE_DIR);
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create cache directory {}", cache_dir.display()))?;
    sled::open(cache_dir.join("sled")).context("failed to open cache database")
}

fn read_cached_string(db: &sled::Db, key: &str) -> Result<Option<String>> {
    let value = db
        .get(key.as_bytes())
        .with_context(|| format!("failed to read cache key {}", key))?;
    if let Some(bytes) = value {
        let string = String::from_utf8(bytes.to_vec())
            .with_context(|| format!("cache entry for {} is not valid utf-8", key))?;
        Ok(Some(string))
    } else {
        Ok(None)
    }
}

fn store_cached_string(db: &sled::Db, key: &str, value: &str) -> Result<()> {
    db.insert(key.as_bytes(), value.as_bytes())
        .with_context(|| format!("failed to update cache key {}", key))?;
    Ok(())
}

pub fn render_site(root: &Path, plan: RenderPlan) -> Result<()> {
    let config_path = root.join("bucket3.yaml");
    let config_raw = if config_path.exists() {
        fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config file {}", config_path.display()))?
    } else {
        String::new()
    };
    let config = Config::load(&config_path)?;
    let html_root = root.join("html");
    fs::create_dir_all(&html_root).context("failed to ensure html directory exists")?;

    let cache_db = open_cache_db(root)?;
    let mut env = template::environment(&config)?;
    let template_hash = load_templates(root, &mut env)?;
    let site_inputs_hash = compute_site_inputs_hash(&config_raw, &template_hash);

    let stored_site_hash = read_cached_string(&cache_db, SITE_INPUTS_KEY)?;
    let site_changed = stored_site_hash.as_deref() != Some(site_inputs_hash.as_str());

    if plan.verbose {
        if plan.mode == BuildMode::Full {
            log_status(true, "MODE", "Full rebuild requested");
        } else {
            log_status(true, "MODE", "Incremental rebuild requested");
        }
    }

    let effective_mode = match plan.mode {
        BuildMode::Full => BuildMode::Full,
        BuildMode::Changed => {
            if site_changed {
                log_status(
                    plan.verbose,
                    "MODE",
                    "Config or templates changed; forcing full rebuild",
                );
                BuildMode::Full
            } else {
                BuildMode::Changed
            }
        }
    };

    if plan.verbose {
        match effective_mode {
            BuildMode::Full => log_status(true, "MODE", "Executing full rebuild"),
            BuildMode::Changed => log_status(true, "MODE", "Executing incremental rebuild"),
        }
    }

    let cache = HomePageCache::new(cache_db.clone());
    let tag_cache = TagPageCache::new(cache_db.clone());

    let posts = if plan.posts {
        log_status(plan.verbose, "STEP", "Rendering posts");
        let posts = render_posts(
            root,
            &html_root,
            &config,
            &env,
            &cache_db,
            effective_mode,
            plan.verbose,
        )?;
        log_status(
            plan.verbose,
            "STEP",
            format!("Processed {} posts", posts.len()),
        );
        posts
    } else {
        log_status(plan.verbose, "STEP", "Skipping post rendering");
        Vec::new()
    };

    if plan.posts {
        log_status(plan.verbose, "STEP", "Rendering indexes and feeds");
        render_homepage(&posts, &html_root, &config, &env, &cache)?;
        render_tag_archives(&posts, &html_root, &config, &env, &tag_cache)?;
        render_archives(&posts, &html_root, &config, &env)?;
        render_feeds(&posts, &html_root, &config, &env)?;
        store_cached_string(&cache_db, SITE_INPUTS_KEY, &site_inputs_hash)?;
    }

    render_pages(root, &html_root, &env, plan.verbose)?;

    if plan.static_assets {
        let static_hash = compute_static_digest(root)?;
        let stored_static_hash = read_cached_string(&cache_db, STATIC_HASH_KEY)?;
        let static_changed = stored_static_hash.as_deref() != Some(static_hash.as_str());
        let should_copy_static = matches!(effective_mode, BuildMode::Full) || static_changed;
        if should_copy_static {
            log_status(plan.verbose, "STATIC", "Copying static assets");
            copy_static_assets(root, &html_root)?;
        } else {
            log_status(plan.verbose, "STATIC", "Static assets unchanged");
        }
        store_cached_string(&cache_db, STATIC_HASH_KEY, &static_hash)?;
    } else {
        log_status(plan.verbose, "STATIC", "Skipping static assets");
    }

    cache_db.flush().context("failed to flush cache database")?;

    log_status(plan.verbose, "DONE", "Render complete");

    Ok(())
}

fn compute_site_inputs_hash(config_raw: &str, template_hash: &str) -> String {
    let mut hasher = Hasher::new();
    hasher.update(config_raw.as_bytes());
    hasher.update(template_hash.as_bytes());
    hasher.finalize().to_hex().to_string()
}

fn load_templates(root: &Path, env: &mut Environment<'static>) -> Result<String> {
    let templates_dir = root.join("templates");
    if !templates_dir.exists() {
        bail!("templates directory {} not found", templates_dir.display());
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(&templates_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    files.sort();

    let mut hasher = Hasher::new();

    for path in files {
        let template_body = fs::read_to_string(&path)
            .with_context(|| format!("failed to read template {}", path.display()))?;
        let relative_path = path.strip_prefix(&templates_dir).unwrap();
        let relative_name = normalize_path(relative_path);
        hasher.update(relative_name.as_bytes());
        hasher.update(template_body.as_bytes());
        let name_static = Box::leak(relative_name.clone().into_boxed_str());
        let template_static = Box::leak(template_body.into_boxed_str());
        env.add_template(name_static, template_static)
            .with_context(|| format!("failed to register template {}", relative_name))?;
    }

    Ok(hasher.finalize().to_hex().to_string())
}

fn render_posts(
    root: &Path,
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache_db: &sled::Db,
    mode: BuildMode,
    verbose: bool,
) -> Result<Vec<Post>> {
    let posts_dir = root.join("posts");
    let mut posts = discover_posts(&posts_dir, config)?;
    if posts.is_empty() {
        return Ok(posts);
    }

    posts.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.slug.cmp(&b.slug)));

    let post_template = env
        .get_template("post.html")
        .context("post.html template missing")?;

    let mut cache_keys: BTreeSet<String> = BTreeSet::new();

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
            let render_target = html_root.join(post.permalink.trim_start_matches('/'));
            let output_path = render_target.join("index.html");
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }

            let context = build_post_context(config, post)?;
            let rendered = post_template
                .render(minijinja::context! { post => context })
                .with_context(|| format!("failed to render template for {}", post.slug))?;

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

    Ok(posts)
}

fn render_pages(
    root: &Path,
    html_root: &Path,
    env: &Environment<'static>,
    verbose: bool,
) -> Result<()> {
    let pages_dir = root.join("pages");
    if !pages_dir.exists() {
        return Ok(());
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(&pages_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let path = entry.into_path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("html"))
            {
                files.push(path);
            }
        }
    }

    files.sort();

    for path in files {
        let relative = path.strip_prefix(&pages_dir).unwrap();
        let output_path = html_root.join(relative);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read page template {}", path.display()))?;

        let rendered = env
            .render_str(&source, context! {})
            .with_context(|| format!("failed to render page {}", path.display()))?;

        fs::write(&output_path, rendered)
            .with_context(|| format!("failed to write page {}", output_path.display()))?;

        log_status(
            verbose,
            "PAGE",
            format!("Rendered {}", normalize_path(relative)),
        );
    }

    Ok(())
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

fn render_homepage(
    posts: &[Post],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache: &HomePageCache,
) -> Result<()> {
    if posts.is_empty() {
        cache.store_pages(&[])?;
        return Ok(());
    }

    let template = env
        .get_template("index.html")
        .context("index.html template missing")?;

    let per_page = std::cmp::max(1, config.homepage_posts);
    let mut lookup: HashMap<String, &Post> = HashMap::new();
    for post in posts {
        lookup.insert(post_key(post), post);
    }

    let head_slice: Vec<&Post> = posts.iter().take(per_page).collect();
    if head_slice.is_empty() {
        return Ok(());
    }
    let head_cursor = page_cursor(head_slice.last().unwrap());
    let head_posts: Vec<String> = head_slice.iter().map(|p| post_key(p)).collect();

    let mut stored_pages = cache.load_pages()?;
    let previous_head_cursor = stored_pages.first().map(|page| page.cursor.clone());

    stored_pages.retain(|page| page.cursor != head_cursor);

    let mut new_records = Vec::new();
    new_records.push(StoredPage {
        cursor: head_cursor.clone(),
        posts: head_posts.clone(),
    });

    let mut known_ids: HashSet<String> = stored_pages
        .iter()
        .flat_map(|page| page.posts.iter().cloned())
        .collect();
    for id in &head_posts {
        known_ids.insert(id.clone());
    }

    let mut buffer: Vec<&Post> = Vec::new();
    let mut extra_chunks: Vec<Vec<&Post>> = Vec::new();
    for post in posts.iter().skip(per_page) {
        let id = post_key(post);
        if known_ids.contains(&id) {
            continue;
        }
        buffer.push(post);
        known_ids.insert(id);
        if buffer.len() == per_page {
            extra_chunks.push(buffer.clone());
            buffer.clear();
        }
    }
    if !buffer.is_empty() {
        extra_chunks.push(buffer);
    }

    let mut seen_cursors: HashSet<String> = HashSet::new();
    seen_cursors.insert(head_cursor.clone());

    for chunk in &extra_chunks {
        if let Some(last) = chunk.last() {
            let cursor = page_cursor(last);
            if seen_cursors.insert(cursor.clone()) {
                let ids = chunk.iter().map(|p| post_key(p)).collect::<Vec<_>>();
                new_records.push(StoredPage { cursor, posts: ids });
            }
        }
    }

    for page in stored_pages.iter() {
        if seen_cursors.insert(page.cursor.clone()) {
            new_records.push(page.clone());
        }
    }

    let head_changed = previous_head_cursor
        .map(|cursor| cursor != head_cursor)
        .unwrap_or(true);

    let mut plans: Vec<PagePlan> = Vec::new();

    for (index, record) in new_records.iter().enumerate() {
        let summaries = record
            .posts
            .iter()
            .filter_map(|id| lookup.get(id))
            .map(|post| build_post_summary(config, post))
            .collect::<Result<Vec<_>>>()?;

        let prev = if index == 0 {
            String::new()
        } else if index == 1 {
            "/".to_string()
        } else {
            page_url(&new_records[index - 1].cursor)
        };
        let next = if index + 1 < new_records.len() {
            page_url(&new_records[index + 1].cursor)
        } else {
            String::new()
        };

        let pagination = PaginationContext {
            current: index + 1,
            total: new_records.len(),
            prev,
            next,
        };

        let outputs = if index == 0 {
            vec![html_root.join("index.html")]
        } else {
            vec![page_output_path(html_root, &record.cursor)]
        };

        let exists_previously = stored_pages.iter().any(|page| page.cursor == record.cursor);
        let mut needs_render = index == 0 || !exists_previously;
        if !needs_render {
            if head_changed && index == 1 {
                needs_render = true;
            } else {
                let path = page_output_path(html_root, &record.cursor);
                needs_render = !path.exists();
            }
        }

        if needs_render {
            plans.push(PagePlan {
                summaries,
                pagination,
                outputs,
            });
        }
    }

    for plan in plans {
        render_page(&template, plan)?;
    }

    cache.store_pages(&new_records)?;

    Ok(())
}

fn render_archives(
    posts: &[Post],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
) -> Result<()> {
    if posts.is_empty() {
        return Ok(());
    }

    let year_template = env
        .get_template("archive_year.html")
        .context("archive_year.html template missing")?;
    let month_template = env
        .get_template("archive_month.html")
        .context("archive_month.html template missing")?;

    let mut year_map: HashMap<i32, Vec<&Post>> = HashMap::new();
    let mut month_map: HashMap<(i32, u8), Vec<&Post>> = HashMap::new();

    for post in posts {
        let year = post.date.year();
        year_map.entry(year).or_default().push(post);

        let month = u8::from(post.date.month());
        month_map.entry((year, month)).or_default().push(post);
    }

    let mut years: Vec<i32> = year_map.keys().copied().collect();
    years.sort_by(|a, b| b.cmp(a));

    for year in years {
        if let Some(posts) = year_map.get(&year) {
            let summaries = posts
                .iter()
                .map(|post| build_post_summary(config, post))
                .collect::<Result<Vec<_>>>()?;

            let rendered = year_template
                .render(minijinja::context! { year => year, posts => summaries })
                .context("failed to render year archive")?;

            let output = archive_year_path(html_root, year);
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&output, rendered)
                .with_context(|| format!("failed to write {}", output.display()))?;
        }
    }

    let mut months: Vec<(i32, u8)> = month_map.keys().copied().collect();
    months.sort_by(|a, b| b.cmp(a));

    for (year, month) in months {
        if let Some(posts) = month_map.get(&(year, month)) {
            let summaries = posts
                .iter()
                .map(|post| build_post_summary(config, post))
                .collect::<Result<Vec<_>>>()?;

            let rendered = month_template
                .render(minijinja::context! { year => year, month => month, posts => summaries })
                .context("failed to render month archive")?;

            let output = archive_month_path(html_root, year, month);
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&output, rendered)
                .with_context(|| format!("failed to write {}", output.display()))?;
        }
    }

    Ok(())
}

fn render_tag_archives(
    posts: &[Post],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache: &TagPageCache,
) -> Result<()> {
    let mut lookup: HashMap<String, &Post> = HashMap::new();
    for post in posts {
        lookup.insert(post_key(post), post);
    }

    let per_page = std::cmp::max(1, config.homepage_posts);
    let tag_template = env
        .get_template("tag.html")
        .context("tag.html template missing")?;

    let mut buckets: BTreeMap<String, TagBucket> = BTreeMap::new();
    for (idx, post) in posts.iter().enumerate() {
        let mut seen = HashSet::new();
        for tag in &post.tags {
            let tag = tag.trim();
            if tag.is_empty() {
                continue;
            }
            let slug = tag_slug(tag);
            if !seen.insert(slug.clone()) {
                continue;
            }
            let bucket = buckets.entry(slug.clone()).or_insert_with(|| TagBucket {
                name: tag.to_string(),
                slug: slug.clone(),
                indices: Vec::new(),
            });
            bucket.indices.push(idx);
        }
    }

    if buckets.is_empty() {
        cache.store_pages(&[])?;
        return Ok(());
    }

    if !config.paginate_tags {
        for bucket in buckets.values() {
            let summaries = bucket
                .indices
                .iter()
                .map(|&idx| build_post_summary(config, &posts[idx]))
                .collect::<Result<Vec<_>>>()?;
            let plan = TagPagePlan {
                tag: bucket.name.clone(),
                summaries,
                pagination: PaginationContext {
                    current: 1,
                    total: 1,
                    prev: String::new(),
                    next: String::new(),
                },
                outputs: vec![tag_index_path(html_root, &bucket.slug)],
            };
            render_tag_page(&tag_template, plan)?;
        }
        cache.store_pages(&[])?;
        return Ok(());
    }

    let mut stored = cache.load_pages()?;
    stored.sort_by(|a, b| a.slug.cmp(&b.slug).then_with(|| a.cursor.cmp(&b.cursor)));
    let mut stored_map: HashMap<String, Vec<StoredTagPage>> = HashMap::new();
    for page in stored {
        stored_map.entry(page.slug.clone()).or_default().push(page);
    }

    let mut all_records: Vec<StoredTagPage> = Vec::new();
    let mut plans: Vec<TagPagePlan> = Vec::new();

    for bucket in buckets.values() {
        let mut existing = stored_map.remove(&bucket.slug).unwrap_or_default();
        if bucket.indices.is_empty() {
            continue;
        }

        let head_indices: Vec<usize> = bucket.indices.iter().take(per_page).cloned().collect();
        if head_indices.is_empty() {
            continue;
        }

        let head_posts: Vec<&Post> = head_indices.iter().map(|&idx| &posts[idx]).collect();
        let head_cursor = page_cursor(head_posts.last().unwrap());
        let head_ids: Vec<String> = head_posts.iter().map(|post| post_key(post)).collect();

        let existing_head_cursor = existing.first().map(|page| page.cursor.clone());
        let head_changed = existing_head_cursor
            .map(|cursor| cursor != head_cursor)
            .unwrap_or(true);
        let existing_cur_set: HashSet<String> =
            existing.iter().map(|page| page.cursor.clone()).collect();

        let mut records: Vec<StoredTagPage> = Vec::new();
        records.push(StoredTagPage {
            tag: bucket.name.clone(),
            slug: bucket.slug.clone(),
            cursor: head_cursor.clone(),
            posts: head_ids.clone(),
        });

        let mut known_ids: HashSet<String> = head_ids.iter().cloned().collect();

        existing.retain(|page| page.cursor != head_cursor);
        existing.retain(|page| page.posts.iter().all(|id| lookup.contains_key(id)));

        for page in &existing {
            for id in &page.posts {
                known_ids.insert(id.clone());
            }
        }

        let mut buffer: Vec<&Post> = Vec::new();
        for &idx in &bucket.indices {
            let post = &posts[idx];
            let id = post_key(post);
            if known_ids.contains(&id) {
                continue;
            }
            buffer.push(post);
            known_ids.insert(id);
            if buffer.len() == per_page {
                let cursor = page_cursor(buffer.last().unwrap());
                let ids = buffer.iter().map(|p| post_key(p)).collect();
                records.push(StoredTagPage {
                    tag: bucket.name.clone(),
                    slug: bucket.slug.clone(),
                    cursor,
                    posts: ids,
                });
                buffer.clear();
            }
        }
        if !buffer.is_empty() {
            let cursor = page_cursor(buffer.last().unwrap());
            let ids = buffer.iter().map(|p| post_key(p)).collect();
            records.push(StoredTagPage {
                tag: bucket.name.clone(),
                slug: bucket.slug.clone(),
                cursor,
                posts: ids,
            });
        }

        records.extend(existing.into_iter());

        for (index, record) in records.iter().enumerate() {
            let summaries = record
                .posts
                .iter()
                .filter_map(|id| lookup.get(id))
                .map(|post| build_post_summary(config, post))
                .collect::<Result<Vec<_>>>()?;

            let prev = if index == 0 {
                String::new()
            } else if index == 1 {
                tag_index_url(&record.slug)
            } else {
                tag_page_url(&record.slug, &records[index - 1].cursor)
            };

            let next = if index + 1 < records.len() {
                tag_page_url(&record.slug, &records[index + 1].cursor)
            } else {
                String::new()
            };

            let outputs = if index == 0 {
                vec![tag_index_path(html_root, &record.slug)]
            } else {
                vec![tag_page_path(html_root, &record.slug, &record.cursor)]
            };

            let mut needs_render = index == 0 || !existing_cur_set.contains(&record.cursor);
            if !needs_render && head_changed && index == 1 {
                needs_render = true;
            }
            if !needs_render {
                needs_render = !outputs[0].as_path().exists();
            }

            if needs_render {
                plans.push(TagPagePlan {
                    tag: record.tag.clone(),
                    summaries,
                    pagination: PaginationContext {
                        current: index + 1,
                        total: records.len(),
                        prev: prev.clone(),
                        next: next.clone(),
                    },
                    outputs,
                });
            }
        }

        all_records.extend(records);
    }

    for plan in plans {
        render_tag_page(&tag_template, plan)?;
    }

    cache.store_pages(&all_records)?;
    Ok(())
}

fn render_tag_page(template: &minijinja::Template<'_, '_>, plan: TagPagePlan) -> Result<()> {
    let rendered = template
        .render(minijinja::context! { tag => plan.tag, posts => plan.summaries, pagination => plan.pagination })
        .context("failed to render tag page")?;

    for output in plan.outputs {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&output, &rendered)
            .with_context(|| format!("failed to write {}", output.display()))?;
    }

    Ok(())
}

fn render_page(template: &minijinja::Template<'_, '_>, plan: PagePlan) -> Result<()> {
    let rendered = template
        .render(minijinja::context! { posts => plan.summaries, pagination => plan.pagination })
        .context("failed to render homepage")?;

    for output in plan.outputs {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&output, &rendered)
            .with_context(|| format!("failed to write {}", output.display()))?;
    }

    Ok(())
}

fn build_post_context(config: &Config, post: &Post) -> Result<PostTemplate> {
    let display_date = format_date(config, &post.date)?;
    let iso_date = post
        .date
        .format(&Rfc3339)
        .context("failed to format RFC3339 date")?;

    let attached = convert_paths(&post.attached)?;

    let body = att_to_absolute(
        &post.body_html,
        &post.permalink,
        &config.base_url,
        &post.attached,
    );

    Ok(PostTemplate {
        title: post.title.clone(),
        slug: post.slug.clone(),
        date: display_date,
        date_iso: iso_date,
        tags: post.tags.clone(),
        abstract_text: post.abstract_text.clone(),
        attached,
        body,
        excerpt: post.excerpt.clone(),
        permalink: post.permalink.clone(),
        extra: post.extra.clone(),
    })
}

fn build_post_summary(config: &Config, post: &Post) -> Result<PostSummary> {
    let date = format_date(config, &post.date)?;
    let date_iso = post
        .date
        .format(&Rfc3339)
        .context("failed to format RFC3339 date")?;

    let body = att_to_absolute(
        &post.body_html,
        &post.permalink,
        &config.base_url,
        &post.attached,
    );

    Ok(PostSummary {
        title: post.title.clone(),
        slug: post.slug.clone(),
        date,
        date_iso,
        body,
        excerpt: post.excerpt.clone(),
        permalink: post.permalink.clone(),
        extra: post.extra.clone(),
    })
}

fn build_feed_item(config: &Config, post: &Post) -> Result<FeedItem> {
    let item_title = post
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&post.slug);
    let link = absolute_url(&config.base_url, &post.permalink);
    let pub_date = format_rfc2822(&post.date)?;
    let description = if post.excerpt.trim().is_empty() {
        item_title.to_string()
    } else {
        post.excerpt.clone()
    };

    let body = att_to_absolute(
        &post.body_html,
        &post.permalink,
        &config.base_url,
        &post.attached,
    );

    Ok(FeedItem {
        title: xml_escape(item_title),
        link: xml_escape(&link),
        guid: xml_escape(&link),
        pub_date: xml_escape(&pub_date),
        description: xml_escape(&description),
        content: sanitize_cdata(&body),
    })
}

fn format_date(config: &Config, date: &OffsetDateTime) -> Result<String> {
    if config.date_format.eq_ignore_ascii_case("RFC3339") {
        return date
            .format(&Rfc3339)
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

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|comp| comp.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
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

fn att_to_absolute(body: &str, permalink: &str, base_url: &str, attached: &[PathBuf]) -> String {
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
                rewrite_if_attached(value, permalink, base_url, &attached_paths)
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

    let base = join_permalink(permalink, path_part);
    let joined = if suffix.is_empty() {
        base
    } else {
        format!("{}{}", base, suffix)
    };
    Some(absolute_url(base_url, &joined))
}

fn post_key(post: &Post) -> String {
    format!("{}-{}", post.date.unix_timestamp(), post.slug)
}

fn page_cursor(post: &Post) -> String {
    post_key(post)
}

fn page_url(cursor: &str) -> String {
    format!("/page/{}/", cursor)
}

fn page_output_path(html_root: &Path, cursor: &str) -> PathBuf {
    html_root.join("page").join(cursor).join("index.html")
}

fn tag_slug(tag: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in tag.chars() {
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

    if slug.is_empty() {
        "untagged".to_string()
    } else {
        slug
    }
}

fn tag_index_path(html_root: &Path, slug: &str) -> PathBuf {
    html_root.join("tags").join(slug).join("index.html")
}

fn tag_page_path(html_root: &Path, slug: &str, cursor: &str) -> PathBuf {
    html_root
        .join("tags")
        .join(slug)
        .join(cursor)
        .join("index.html")
}

fn tag_index_url(slug: &str) -> String {
    format!("/tags/{}/", slug)
}

fn tag_page_url(slug: &str, cursor: &str) -> String {
    format!("/tags/{}/{}/", slug, cursor)
}

fn archive_year_path(html_root: &Path, year: i32) -> PathBuf {
    html_root.join(format!("{:04}", year)).join("index.html")
}

fn archive_month_path(html_root: &Path, year: i32, month: u8) -> PathBuf {
    html_root
        .join(format!("{:04}", year))
        .join(format!("{:02}", month))
        .join("index.html")
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

fn compute_static_digest(root: &Path) -> Result<String> {
    let skel_dir = root.join("skel");
    if !skel_dir.exists() {
        return Ok(Hasher::new().finalize().to_hex().to_string());
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(&skel_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    files.sort();

    let mut hasher = Hasher::new();
    for path in files {
        let relative = path.strip_prefix(&skel_dir).unwrap();
        let normalized = normalize_path(relative);
        hasher.update(normalized.as_bytes());
        let data = fs::read(&path)
            .with_context(|| format!("failed to read static asset {}", path.display()))?;
        hasher.update(&data);
        let metadata = fs::metadata(&path)
            .with_context(|| format!("failed to inspect static asset {}", path.display()))?;
        hasher.update(&metadata.len().to_le_bytes());
        let modified = metadata.modified().with_context(|| {
            format!(
                "failed to read modification time for static asset {}",
                path.display()
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

fn copy_static_assets(root: &Path, html_root: &Path) -> Result<()> {
    let skel_dir = root.join("skel");
    if !skel_dir.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(&skel_dir) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        let relative = entry.path().strip_prefix(&skel_dir).unwrap();
        let destination = html_root.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(entry.path(), &destination).with_context(|| {
            format!(
                "failed to copy static asset from {} to {}",
                entry.path().display(),
                destination.display()
            )
        })?;
    }

    Ok(())
}

fn render_feeds(
    posts: &[Post],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
) -> Result<()> {
    render_rss(posts, html_root, config, env)?;

    for tag in config_tag_feeds(config) {
        let slug = tag_slug(&tag);
        let tag_posts: Vec<&Post> = posts
            .iter()
            .filter(|post| post.tags.iter().any(|t| t.eq(&tag)))
            .collect();
        let output_path = html_root.join(format!("rss-{}.xml", slug));
        let title = config
            .title
            .clone()
            .unwrap_or_else(|| "bucket3".to_string());
        let feed_title = format!("{} · {}", tag, title);
        let site_path = format!("/tags/{}/", slug);
        let feed_path = format!("/rss-{}.xml", slug);
        render_feed(
            tag_posts,
            config,
            env,
            &site_path,
            &feed_path,
            &output_path,
            Some(feed_title),
        )?;
    }
    render_sitemap(posts, html_root, config)?;
    Ok(())
}

fn render_rss(
    posts: &[Post],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
) -> Result<()> {
    let output_path = html_root.join("rss.xml");
    let posts_ref: Vec<&Post> = posts.iter().collect();
    render_feed(posts_ref, config, env, "/", "/rss.xml", &output_path, None)
}

fn render_feed(
    posts: Vec<&Post>,
    config: &Config,
    env: &Environment<'static>,
    site_path: &str,
    feed_path: &str,
    output_path: &Path,
    title: Option<String>,
) -> Result<()> {
    let template = env
        .get_template("rss.xml")
        .context("rss.xml template missing")?;

    let site_url = absolute_url(&config.base_url, site_path);
    let feed_url = absolute_url(&config.base_url, feed_path);
    let resolved_title = title.unwrap_or_else(|| {
        config
            .title
            .clone()
            .unwrap_or_else(|| "bucket3".to_string())
    });
    let build_date = posts
        .first()
        .map(|post| post.date)
        .unwrap_or_else(OffsetDateTime::now_utc);
    let last_build_date = format_rfc2822(&build_date)?;

    let items = posts
        .into_iter()
        .take(50)
        .map(|post| build_feed_item(config, post))
        .collect::<Result<Vec<_>>>()?;

    let context = FeedContext {
        title: xml_escape(&resolved_title),
        site_url: xml_escape(&site_url),
        feed_url: xml_escape(&feed_url),
        description: xml_escape(&resolved_title),
        updated: xml_escape(&last_build_date),
        items,
    };

    let rendered = template
        .render(minijinja::context! { feed => context })
        .context("failed to render rss.xml")?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(output_path, rendered)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    Ok(())
}

fn render_sitemap(posts: &[Post], html_root: &Path, config: &Config) -> Result<()> {
    let per_page = std::cmp::max(1, config.homepage_posts);
    let mut entries: Vec<SitemapEntry> = Vec::new();

    let homepage_date = posts
        .first()
        .map(|post| format_rfc3339(&post.date))
        .transpose()?;
    entries.push(SitemapEntry {
        loc: absolute_url(&config.base_url, "/"),
        lastmod: homepage_date,
    });

    for chunk in posts.chunks(per_page).enumerate() {
        if chunk.0 == 0 {
            continue;
        }
        let last = chunk.1.last().expect("chunks() never yields empty slices");
        let cursor = page_cursor(last);
        let path = page_url(&cursor);
        let chunk_date = format_rfc3339(&chunk.1[0].date)?;
        entries.push(SitemapEntry {
            loc: absolute_url(&config.base_url, &path),
            lastmod: Some(chunk_date),
        });
    }

    for post in posts {
        entries.push(SitemapEntry {
            loc: absolute_url(&config.base_url, &post.permalink),
            lastmod: Some(format_rfc3339(&post.date)?),
        });
    }

    let tag_entries = collect_tag_sitemap_entries(posts, config, per_page)?;
    entries.extend(tag_entries);

    let mut buffer = String::new();
    writeln!(buffer, "<?xml version=\"1.0\" encoding=\"utf-8\"?>")?;
    writeln!(
        buffer,
        "<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">"
    )?;
    for entry in entries {
        writeln!(buffer, "  <url>")?;
        writeln!(buffer, "    <loc>{}</loc>", xml_escape(&entry.loc))?;
        if let Some(lastmod) = entry.lastmod {
            writeln!(buffer, "    <lastmod>{}</lastmod>", xml_escape(&lastmod))?;
        }
        writeln!(buffer, "  </url>")?;
    }
    writeln!(buffer, "</urlset>")?;

    let output_path = html_root.join("sitemap.xml");
    fs::write(&output_path, buffer)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    Ok(())
}

fn collect_tag_sitemap_entries(
    posts: &[Post],
    config: &Config,
    per_page: usize,
) -> Result<Vec<SitemapEntry>> {
    let mut buckets: BTreeMap<String, TagBucket> = BTreeMap::new();

    for (idx, post) in posts.iter().enumerate() {
        let mut seen = HashSet::new();
        for tag in &post.tags {
            let tag = tag.trim();
            if tag.is_empty() {
                continue;
            }
            let slug = tag_slug(tag);
            if !seen.insert(slug.clone()) {
                continue;
            }
            let bucket = buckets.entry(slug.clone()).or_insert_with(|| TagBucket {
                name: tag.to_string(),
                slug: slug.clone(),
                indices: Vec::new(),
            });
            bucket.indices.push(idx);
        }
    }

    if buckets.is_empty() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for bucket in buckets.values() {
        if !config.paginate_tags {
            let first = &posts[bucket.indices[0]];
            entries.push(SitemapEntry {
                loc: absolute_url(&config.base_url, &tag_index_url(&bucket.slug)),
                lastmod: Some(format_rfc3339(&first.date)?),
            });
            continue;
        }

        for (chunk_index, chunk) in bucket.indices.chunks(per_page).enumerate() {
            let first = &posts[chunk[0]];
            let url = if chunk_index == 0 {
                tag_index_url(&bucket.slug)
            } else {
                let last = &posts[*chunk.last().unwrap()];
                let cursor = page_cursor(last);
                tag_page_url(&bucket.slug, &cursor)
            };
            entries.push(SitemapEntry {
                loc: absolute_url(&config.base_url, &url),
                lastmod: Some(format_rfc3339(&first.date)?),
            });
        }
    }

    Ok(entries)
}

fn format_rfc3339(date: &OffsetDateTime) -> Result<String> {
    date.format(&Rfc3339)
        .context("failed to format RFC3339 date")
}

fn format_rfc2822(date: &OffsetDateTime) -> Result<String> {
    date.format(&Rfc2822)
        .context("failed to format RFC2822 date")
}

fn sanitize_cdata(value: &str) -> String {
    if value.contains("]]>") {
        value.replace("]]>", "]]]><![CDATA[>")
    } else {
        value.to_string()
    }
}

fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

struct SitemapEntry {
    loc: String,
    lastmod: Option<String>,
}

#[derive(Serialize)]
struct FeedContext {
    title: String,
    site_url: String,
    feed_url: String,
    description: String,
    updated: String,
    items: Vec<FeedItem>,
}

#[derive(Serialize)]
struct FeedItem {
    title: String,
    link: String,
    guid: String,
    pub_date: String,
    description: String,
    content: String,
}

#[derive(Serialize)]
struct PostTemplate {
    title: Option<String>,
    slug: String,
    date: String,
    date_iso: String,
    tags: Vec<String>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    attached: Vec<String>,
    body: String,
    excerpt: String,
    permalink: String,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize)]
struct PostSummary {
    title: Option<String>,
    slug: String,
    date: String,
    date_iso: String,
    body: String,
    excerpt: String,
    permalink: String,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize)]
struct PaginationContext {
    current: usize,
    total: usize,
    prev: String,
    next: String,
}

struct TagPagePlan {
    tag: String,
    summaries: Vec<PostSummary>,
    pagination: PaginationContext,
    outputs: Vec<PathBuf>,
}

struct PagePlan {
    summaries: Vec<PostSummary>,
    pagination: PaginationContext,
    outputs: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    fn write_template(root: &Path, name: &str, contents: &str) {
        let path = root.join("templates").join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn setup_markdown_templates(root: &Path) {
        write_template(
            root,
            "base.html",
            "<!doctype html><html><body>{% block content %}{% endblock %}</body></html>",
        );
        write_template(
            root,
            "post.html",
            "{% extends \"base.html\" %}{% block content %}<article>{{ post.title }}|{{ post.body | safe }}|{{ post.date }}|{{ post.excerpt }}</article>{% endblock %}",
        );
        write_template(
            root,
            "index.html",
            "{% extends \"base.html\" %}{% block content %}<section data-current=\"{{ pagination.current }}\" data-total=\"{{ pagination.total }}\" data-prev=\"{{ pagination.prev | safe }}\" data-next=\"{{ pagination.next | safe }}\">{% for post in posts %}<article data-slug=\"{{ post.slug }}\"></article>{% endfor %}</section>{% endblock %}",
        );
        write_template(
            root,
            "tag.html",
            "{% extends \"base.html\" %}{% block content %}<section data-tag=\"{{ tag }}\" data-current=\"{{ pagination.current }}\" data-total=\"{{ pagination.total }}\" data-prev=\"{{ pagination.prev | safe }}\" data-next=\"{{ pagination.next | safe }}\">{% for post in posts %}<article data-slug=\"{{ post.slug }}\"></article>{% endfor %}</section>{% endblock %}",
        );
        write_template(
            root,
            "archive_year.html",
            "{% extends \"base.html\" %}{% block content %}<section data-year=\"{{ year }}\">{% for post in posts %}<article data-slug=\"{{ post.slug }}\"></article>{% endfor %}</section>{% endblock %}",
        );
        write_template(
            root,
            "archive_month.html",
            "{% extends \"base.html\" %}{% block content %}<section data-year=\"{{ year }}\" data-month=\"{{ month }}\">{% for post in posts %}<article data-slug=\"{{ post.slug }}\"></article>{% endfor %}</section>{% endblock %}",
        );
        write_template(
            root,
            "rss.xml",
            "{% autoescape false %}\n<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<rss version=\"2.0\" xmlns:content=\"http://purl.org/rss/1.0/modules/content/\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n  <channel>\n    <title>{{ feed.title }}</title>\n    <link>{{ feed.site_url }}</link>\n    <description>{{ feed.description }}</description>\n    <lastBuildDate>{{ feed.updated }}</lastBuildDate>\n    <generator>bucket3</generator>\n    <atom:link href=\"{{ feed.feed_url }}\" rel=\"self\" type=\"application/rss+xml\"/>\n    {% for item in feed.items %}\n    <item>\n      <title>{{ item.title }}</title>\n      <link>{{ item.link }}</link>\n      <guid isPermaLink=\"true\">{{ item.guid }}</guid>\n      <pubDate>{{ item.pub_date }}</pubDate>\n      <description>{{ item.description }}</description>\n      <content:encoded><![CDATA[{{ item.content | safe }}]]></content:encoded>\n    </item>\n    {% endfor %}\n  </channel>\n</rss>\n{% endautoescape %}\n",
        );
    }

    fn write_markdown_post(root: &Path, body: &str) {
        let post_dir = root.join("posts/hello-world");
        fs::create_dir_all(&post_dir).unwrap();
        fs::write(
            post_dir.join("post.md"),
            format!(
                "---\ntitle: Example\ndate: 2024-01-02T03:04:05Z\ntags: [test]\n---\n{}",
                body
            ),
        )
        .unwrap();
    }

    fn write_tagged_post(root: &Path, slug: &str, tag: &str, date: &str, body: &str) {
        let dir = root.join("posts").join(slug);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("post.md"),
            format!(
                "---\ntitle: {0}\ndate: {2}\nslug: {0}\ntags:\n  - {1}\n---\n{3}",
                slug, tag, date, body
            ),
        )
        .unwrap();
    }

    fn write_dated_post(root: &Path, slug: &str, date: &str, body: &str) {
        let dir = root.join("posts").join(slug);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("post.md"),
            format!(
                "---\ntitle: {0}\ndate: {1}\nslug: {0}\ntags:\n  - {0}\n---\n{2}",
                slug, date, body
            ),
        )
        .unwrap();
    }

    fn file_mtime(path: &Path) -> std::time::Duration {
        fs::metadata(path)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(super::UNIX_EPOCH)
            .unwrap()
    }

    fn wait_for_filesystem_tick() {
        std::thread::sleep(std::time::Duration::from_millis(1100));
    }

    #[test]
    fn renders_markdown_post_to_expected_location() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        fs::create_dir_all(root.join("skel")).unwrap();
        setup_markdown_templates(root);
        write_markdown_post(root, "Hello **world**!");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let output = root.join("html/2024/01/02/hello-world/index.html");
        let rendered = fs::read_to_string(output).unwrap();
        assert!(rendered.contains("Example"));
        assert!(rendered.contains("<strong>world</strong>"));
        assert!(rendered.contains("Hello world"));

        let homepage = fs::read_to_string(root.join("html/index.html")).unwrap();
        assert!(homepage.contains("article data-slug=\"hello-world\""));
        assert!(homepage.contains("data-current=\"1\""));
        assert!(homepage.contains("data-total=\"1\""));
    }

    #[test]
    fn copies_post_assets() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts/assets-post")).unwrap();
        setup_markdown_templates(root);
        fs::write(
            root.join("posts/assets-post/post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\nattached: [data/notes.txt, images/pic.png]\n---\nBody",
        )
        .unwrap();
        fs::create_dir_all(root.join("posts/assets-post/data")).unwrap();
        fs::create_dir_all(root.join("posts/assets-post/images")).unwrap();
        fs::write(root.join("posts/assets-post/data/notes.txt"), "notes").unwrap();
        fs::write(root.join("posts/assets-post/images/pic.png"), "image").unwrap();

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let asset = root.join("html/2024/01/01/assets-post/data/notes.txt");
        let image = root.join("html/2024/01/01/assets-post/images/pic.png");
        assert!(asset.exists());
        assert!(image.exists());
    }

    #[test]
    fn renders_pages_from_pages_directory() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        setup_markdown_templates(root);
        fs::create_dir_all(root.join("pages/about")).unwrap();
        fs::write(
            root.join("pages/404.html"),
            "{% extends \"base.html\" %}{% block content %}<h1>Missing</h1>{% endblock %}",
        )
        .unwrap();
        fs::write(
            root.join("pages/about/index.html"),
            "{% extends \"base.html\" %}{% block content %}<p>About {{ config.title | default('site') }}</p>{% endblock %}",
        )
        .unwrap();

        render_site(
            root,
            RenderPlan {
                posts: false,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let not_found = fs::read_to_string(root.join("html/404.html")).unwrap();
        assert!(not_found.contains("Missing"));

        let about = fs::read_to_string(root.join("html/about/index.html")).unwrap();
        assert!(about.contains("About"));
    }

    #[test]
    fn exposes_additional_front_matter_in_templates() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        setup_markdown_templates(root);

        fs::write(
            root.join("templates/post.html"),
            "{% extends \"base.html\" %}{% block content %}<article>{{ post.location.country }}</article>{% endblock %}",
        )
        .unwrap();

        fs::create_dir_all(root.join("posts/location")).unwrap();
        fs::write(
            root.join("posts/location/post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\nlocation:\n  country: GR\n---\nBody",
        )
        .unwrap();

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let rendered =
            fs::read_to_string(root.join("html/2024/01/01/location/index.html")).unwrap();
        assert!(rendered.contains("GR"));
    }

    #[test]
    fn copies_static_assets() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("skel/css")).unwrap();
        fs::write(root.join("skel/css/site.css"), "body { color: black; }").unwrap();
        setup_markdown_templates(root);

        render_site(
            root,
            RenderPlan {
                posts: false,
                static_assets: true,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let copied = root.join("html/css/site.css");
        assert!(copied.exists());
    }

    #[test]
    fn paginates_homepage_cursor_based() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        setup_markdown_templates(root);
        fs::write(root.join("bucket3.yaml"), "homepage_posts: 1\n").unwrap();

        write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "A");
        write_dated_post(root, "beta", "2024-02-01T00:00:00Z", "B");
        write_dated_post(root, "gamma", "2024-03-01T00:00:00Z", "C");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let ts_gamma = OffsetDateTime::parse("2024-03-01T00:00:00Z", &Rfc3339)
            .unwrap()
            .unix_timestamp();
        let ts_beta = OffsetDateTime::parse("2024-02-01T00:00:00Z", &Rfc3339)
            .unwrap()
            .unix_timestamp();
        let ts_alpha = OffsetDateTime::parse("2024-01-01T00:00:00Z", &Rfc3339)
            .unwrap()
            .unix_timestamp();

        let index = fs::read_to_string(root.join("html/index.html")).unwrap();
        assert!(index.contains("article data-slug=\"gamma\""));
        assert!(index.contains(&format!("data-next=\"/page/{ts_beta}-beta/\"")));

        let second =
            fs::read_to_string(root.join(format!("html/page/{ts_beta}-beta/index.html"))).unwrap();
        assert!(second.contains("article data-slug=\"beta\""));
        assert!(second.contains("data-prev=\"/\""));
        assert!(second.contains(&format!("data-next=\"/page/{ts_alpha}-alpha/\"")));

        let third = fs::read_to_string(root.join(format!("html/page/{ts_alpha}-alpha/index.html")))
            .unwrap();
        assert!(third.contains("article data-slug=\"alpha\""));
        assert!(third.contains(&format!("data-prev=\"/page/{ts_beta}-beta/\"")));
        assert!(third.contains("data-next=\"\""));

        // Add a new post and ensure only new pages are added with stable cursors
        write_dated_post(root, "delta", "2024-04-01T00:00:00Z", "D");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let refreshed_index = fs::read_to_string(root.join("html/index.html")).unwrap();
        assert!(refreshed_index.contains("article data-slug=\"delta\""));
        assert!(refreshed_index.contains(&format!("data-next=\"/page/{ts_gamma}-gamma/\"")));

        let archived = root.join(format!("html/page/{ts_beta}-beta/index.html"));
        assert!(archived.exists());
    }

    #[test]
    fn renders_tag_pages_without_pagination() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        setup_markdown_templates(root);
        fs::write(
            root.join("bucket3.yaml"),
            "homepage_posts: 5\npaginate_tags: false\n",
        )
        .unwrap();

        write_tagged_post(root, "first", "shared", "2024-01-01T00:00:00Z", "Body A");
        write_tagged_post(root, "second", "shared", "2024-02-01T00:00:00Z", "Body B");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let tag_root = root.join("html/tags/shared");
        assert!(tag_root.join("index.html").exists());
        assert!(!tag_root.join("first").exists());
    }

    #[test]
    fn renders_tag_pages_with_pagination() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        setup_markdown_templates(root);
        fs::write(
            root.join("bucket3.yaml"),
            "homepage_posts: 1\npaginate_tags: true\n",
        )
        .unwrap();

        write_tagged_post(root, "alpha", "shared", "2024-01-01T00:00:00Z", "A");
        write_tagged_post(root, "beta", "shared", "2024-02-01T00:00:00Z", "B");
        write_tagged_post(root, "gamma", "shared", "2024-03-01T00:00:00Z", "C");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let ts_beta = OffsetDateTime::parse("2024-02-01T00:00:00Z", &Rfc3339)
            .unwrap()
            .unix_timestamp();
        let ts_alpha = OffsetDateTime::parse("2024-01-01T00:00:00Z", &Rfc3339)
            .unwrap()
            .unix_timestamp();

        let tag_index = fs::read_to_string(root.join("html/tags/shared/index.html")).unwrap();
        assert!(tag_index.contains("article data-slug=\"gamma\""));
        assert!(tag_index.contains(&format!("data-next=\"/tags/shared/{ts_beta}-beta/\"")));

        let second =
            fs::read_to_string(root.join(format!("html/tags/shared/{ts_beta}-beta/index.html")))
                .unwrap();
        assert!(second.contains("article data-slug=\"beta\""));
        assert!(second.contains("data-prev=\"/tags/shared/\""));

        let third =
            fs::read_to_string(root.join(format!("html/tags/shared/{ts_alpha}-alpha/index.html")))
                .unwrap();
        assert!(third.contains("article data-slug=\"alpha\""));
        assert!(third.contains(&format!("data-prev=\"/tags/shared/{ts_beta}-beta/\"")));
        assert!(third.contains("data-next=\"\""));
    }

    #[test]
    fn generates_rss_feed_with_absolute_urls() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        setup_markdown_templates(root);
        fs::write(
            root.join("bucket3.yaml"),
            "base_url: \"https://example.com/blog\"\n",
        )
        .unwrap();

        write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "Alpha body");
        write_dated_post(root, "beta", "2024-02-01T00:00:00Z", "Beta body");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let feed = fs::read_to_string(root.join("html/rss.xml")).unwrap();
        assert!(feed.contains("<link>https://example.com/blog/</link>"));
        assert!(feed.contains("<atom:link href=\"https://example.com/blog/rss.xml\""));
        assert!(feed.contains("<link>https://example.com/blog/2024/02/01/beta/</link>"));
        assert!(feed.contains("<description>Beta body"));
        assert!(feed.contains("<content:encoded><![CDATA["));
    }

    #[test]
    fn generates_tag_rss_feeds_when_configured() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        setup_markdown_templates(root);
        fs::write(
            root.join("bucket3.yaml"),
            "title: Demo Site\nbase_url: \"https://example.com\"\nrss_tags:\n  - shared\n",
        )
        .unwrap();

        write_tagged_post(root, "alpha", "shared", "2024-01-01T00:00:00Z", "A");
        write_tagged_post(root, "beta", "other", "2024-02-01T00:00:00Z", "B");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let feed_path = root.join("html/rss-shared.xml");
        assert!(feed_path.exists());
        let feed = fs::read_to_string(feed_path).unwrap();
        assert!(feed.contains("shared · Demo Site"));
        assert!(feed.contains("/2024/01/01/alpha/"));
        assert!(!feed.contains("/2024/02/01/beta/"));
    }

    #[test]
    fn rewrites_relative_asset_urls_to_absolute() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts/media/images")).unwrap();
        setup_markdown_templates(root);
        fs::write(root.join("posts/media/images/pic.png"), "image-bytes").unwrap();
        fs::write(root.join("posts/media/notes.txt"), "notes").unwrap();
        fs::write(
            root.join("posts/media/post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\nattached:\n  - images/pic.png\n  - notes.txt\n---\n![Alt](images/pic.png)\n\n[Download](notes.txt)\n",
        )
        .unwrap();

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let post_page = fs::read_to_string(root.join("html/2024/01/01/media/index.html")).unwrap();
        assert!(post_page.contains("/2024/01/01/media/images/pic.png"));
        assert!(post_page.contains("/2024/01/01/media/notes.txt"));

        let feed = fs::read_to_string(root.join("html/rss.xml")).unwrap();
        assert!(feed.contains("/2024/01/01/media/images/pic.png"));
        assert!(feed.contains("/2024/01/01/media/notes.txt"));
    }

    #[test]
    fn generates_sitemap_with_posts_tags_and_pages() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        setup_markdown_templates(root);
        fs::write(
            root.join("bucket3.yaml"),
            "base_url: \"https://example.com/blog\"\nhomepage_posts: 1\npaginate_tags: true\n",
        )
        .unwrap();

        write_tagged_post(root, "alpha", "shared", "2024-01-01T00:00:00Z", "A");
        write_tagged_post(root, "beta", "shared", "2024-02-01T00:00:00Z", "B");
        write_tagged_post(root, "gamma", "shared", "2024-03-01T00:00:00Z", "C");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        let sitemap = fs::read_to_string(root.join("html/sitemap.xml")).unwrap();
        assert!(sitemap.contains("<loc>https://example.com/blog/</loc>"));

        let ts_beta = OffsetDateTime::parse("2024-02-01T00:00:00Z", &Rfc3339)
            .unwrap()
            .unix_timestamp();
        let ts_alpha = OffsetDateTime::parse("2024-01-01T00:00:00Z", &Rfc3339)
            .unwrap()
            .unix_timestamp();

        assert!(sitemap.contains(&format!(
            "<loc>https://example.com/blog/page/{ts_beta}-beta/</loc>"
        )));
        assert!(sitemap.contains(&format!(
            "<loc>https://example.com/blog/page/{ts_alpha}-alpha/</loc>"
        )));
        assert!(sitemap.contains(&format!("<loc>https://example.com/blog/tags/shared/</loc>")));
        assert!(sitemap.contains(&format!(
            "<loc>https://example.com/blog/tags/shared/{ts_beta}-beta/</loc>"
        )));
        assert!(sitemap.contains("<loc>https://example.com/blog/2024/03/01/gamma/</loc>"));
        assert!(sitemap.contains("<lastmod>2024-03-01T00:00:00Z</lastmod>"));
    }

    #[test]
    fn renders_year_and_month_archives() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        setup_markdown_templates(root);

        write_dated_post(root, "jan", "2023-01-01T00:00:00Z", "Old");
        write_dated_post(root, "feb", "2024-02-01T00:00:00Z", "Mid");
        write_dated_post(root, "mar", "2024-03-01T00:00:00Z", "New");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
                mode: BuildMode::Full,
                verbose: false,
            },
        )
        .unwrap();

        assert!(root.join("html/2024/index.html").exists());
        assert!(root.join("html/2024/03/index.html").exists());
        assert!(root.join("html/2023/index.html").exists());
    }

    #[test]
    fn incremental_rebuilds_only_changed_post() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        setup_markdown_templates(root);

        write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "Alpha body");
        write_dated_post(root, "beta", "2024-02-01T00:00:00Z", "Beta body");

        let alpha_output = root.join("html/2024/01/01/alpha/index.html");
        let beta_output = root.join("html/2024/02/01/beta/index.html");

        let full_plan = RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        };
        let changed_plan = RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        };

        render_site(root, full_plan).unwrap();

        let alpha_first = file_mtime(&alpha_output);
        let beta_first = file_mtime(&beta_output);

        wait_for_filesystem_tick();
        render_site(root, changed_plan).unwrap();

        let alpha_second = file_mtime(&alpha_output);
        let beta_second = file_mtime(&beta_output);
        assert_eq!(alpha_first, alpha_second);
        assert_eq!(beta_first, beta_second);

        wait_for_filesystem_tick();
        write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "Alpha updated");
        render_site(root, changed_plan).unwrap();

        let alpha_third = file_mtime(&alpha_output);
        let beta_third = file_mtime(&beta_output);
        assert!(alpha_third > alpha_second);
        assert_eq!(beta_second, beta_third);
    }

    #[test]
    fn template_change_triggers_full_rebuild() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        setup_markdown_templates(root);

        write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "Alpha body");
        write_dated_post(root, "beta", "2024-02-01T00:00:00Z", "Beta body");

        let alpha_output = root.join("html/2024/01/01/alpha/index.html");
        let beta_output = root.join("html/2024/02/01/beta/index.html");

        let full_plan = RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        };
        let changed_plan = RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        };

        render_site(root, full_plan).unwrap();
        let alpha_initial = file_mtime(&alpha_output);
        let beta_initial = file_mtime(&beta_output);

        wait_for_filesystem_tick();
        render_site(root, changed_plan).unwrap();
        let alpha_after_changed = file_mtime(&alpha_output);
        let beta_after_changed = file_mtime(&beta_output);
        assert_eq!(alpha_initial, alpha_after_changed);
        assert_eq!(beta_initial, beta_after_changed);

        wait_for_filesystem_tick();
        write_template(
            root,
            "base.html",
            "<!doctype html><html><body data-version=\"v2\">{% block content %}{% endblock %}</body></html>",
        );
        render_site(root, changed_plan).unwrap();

        let alpha_after_template = file_mtime(&alpha_output);
        let beta_after_template = file_mtime(&beta_output);
        assert!(alpha_after_template > alpha_after_changed);
        assert!(beta_after_template > beta_after_changed);
    }
}
