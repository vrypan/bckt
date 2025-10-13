use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use minijinja::Environment;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::content::Post;

use super::cache::{read_cached_string, store_cached_string};
use super::posts::{PostSummary, build_post_summary, post_key};
use super::templates::render_template_with_scope;
use super::utils::{compute_cache_digest, log_status, remove_dir_if_empty, remove_file_if_exists};
use super::{
    BuildMode, HOME_PAGES_KEY, MONTH_ARCHIVE_PREFIX, TAG_CACHE_PREFIX, YEAR_ARCHIVE_PREFIX,
};

pub(super) struct HomePageCache {
    db: sled::Db,
}

impl HomePageCache {
    pub(super) fn new(db: sled::Db) -> Self {
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

pub(super) fn render_homepage(
    posts: &[Post],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache: &HomePageCache,
    mode: BuildMode,
) -> Result<()> {
    if posts.is_empty() {
        cache.store_pages(&[])?;
        return Ok(());
    }

    let template = env
        .get_template("index.html")
        .context("index.html template missing")?;

    let per_page = std::cmp::max(1, config.homepage_posts);

    // Posts are now sorted ASCENDING (oldest first, newest last)
    // posts[0] = oldest, posts[len-1] = newest
    // Page 1 gets posts[0..per_page-1], Page 2 gets posts[per_page..2*per_page-1], etc.
    // Homepage gets the last per_page to per_page*2-1 posts (newest)
    // Posts within each page are displayed in reverse (newest first)

    let remainder = posts.len() % per_page;

    // Determine homepage size: between per_page and per_page*2-1 posts
    let home_page_size = if posts.len() < per_page {
        posts.len()
    } else if remainder == 0 {
        per_page
    } else if remainder < per_page {
        remainder + per_page
    } else {
        per_page
    };

    // Number of regular pages
    let regular_page_count = (posts.len() - home_page_size) / per_page;
    let total_pages = regular_page_count + 1;

    let mut new_records = Vec::new();

    // Regular pages (page 1, 2, 3, ...) - store in display order (reversed)
    for page_num in 1..=regular_page_count {
        let start = (page_num - 1) * per_page;
        let end = start + per_page;
        // Reverse the slice to display newest first within the page
        let page_posts: Vec<String> = posts[start..end].iter().rev().map(post_key).collect();
        new_records.push(StoredPage {
            page_number: page_num,
            posts: page_posts,
        });
    }

    // Homepage gets the last posts (newest) - store in display order (reversed)
    let home_start = regular_page_count * per_page;
    let home_posts: Vec<String> = posts[home_start..].iter().rev().map(post_key).collect();
    new_records.push(StoredPage {
        page_number: 0,
        posts: home_posts,
    });

    // Load cached pages to detect changes
    let stored_pages = cache.load_pages()?;
    let mut stored_map: HashMap<usize, &StoredPage> = HashMap::new();
    for page in &stored_pages {
        stored_map.insert(page.page_number, page);
    }

    // Build lookup for rendering
    let mut lookup: HashMap<String, &Post> = HashMap::new();
    for post in posts {
        lookup.insert(post_key(post), post);
    }

    let mut plans: Vec<PagePlan> = Vec::new();

    for record in &new_records {
        let page_num = record.page_number;

        // Check if this page needs rendering
        let mut needs_render = matches!(mode, BuildMode::Full);
        if !needs_render {
            needs_render = match stored_map.get(&page_num) {
                Some(cached) => {
                    // Page exists in cache - check if content changed
                    cached.posts != record.posts
                }
                None => {
                    // New page
                    true
                }
            };
        }

        if !needs_render {
            continue;
        }

        // Build post summaries
        let summaries = record
            .posts
            .iter()
            .filter_map(|id| lookup.get(id))
            .map(|post| build_post_summary(config, post))
            .collect::<Result<Vec<_>>>()?;

        // Build pagination links
        let (prev, next) = if page_num == 0 {
            // Homepage
            let prev = if regular_page_count > 0 {
                page_url(regular_page_count)
            } else {
                String::new()
            };
            (prev, String::new())
        } else if page_num == 1 {
            // Page 1
            let next = if page_num < regular_page_count {
                page_url(page_num + 1)
            } else {
                "/".to_string() // Link to homepage
            };
            (String::new(), next)
        } else {
            // Middle pages
            let prev = page_url(page_num - 1);
            let next = if page_num < regular_page_count {
                page_url(page_num + 1)
            } else {
                "/".to_string() // Link to homepage
            };
            (prev, next)
        };

        let pagination = PaginationContext {
            current: if page_num == 0 { total_pages } else { page_num },
            total: total_pages,
            prev,
            next,
        };

        let output = if page_num == 0 {
            html_root.join("index.html")
        } else {
            page_output_path(html_root, page_num)
        };

        plans.push(PagePlan {
            summaries,
            pagination,
            outputs: vec![output],
        });
    }

    for plan in plans {
        render_page(&template, plan)?;
    }

    cache.store_pages(&new_records)?;

    // Cleanup stale page directories
    cleanup_homepage_pages(html_root, &new_records)?;

    Ok(())
}

pub(super) fn render_archives(
    posts: &[Post],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache_db: &sled::Db,
    mode: BuildMode,
    verbose: bool,
) -> Result<()> {
    let year_template = env
        .get_template("archive_year.html")
        .context("archive_year.html template missing")?;
    let month_template = env
        .get_template("archive_month.html")
        .context("archive_month.html template missing")?;

    let mut year_groups: BTreeMap<i32, Vec<&Post>> = BTreeMap::new();
    let mut month_groups: BTreeMap<(i32, u8), Vec<&Post>> = BTreeMap::new();

    for post in posts {
        year_groups.entry(post.date.year()).or_default().push(post);
        month_groups
            .entry((post.date.year(), post.date.month() as u8))
            .or_default()
            .push(post);
    }

    let mut year_keys: BTreeSet<String> = BTreeSet::new();
    for (year, group) in year_groups.iter().rev() {
        let summaries = group
            .iter()
            .map(|post| build_post_summary(config, post))
            .collect::<Result<Vec<_>>>()?;
        let payload = YearArchiveCachePayload {
            year: *year,
            posts: &summaries,
        };
        let digest = compute_cache_digest(&payload)?;
        let cache_key = format!("{YEAR_ARCHIVE_PREFIX}{year:04}");
        year_keys.insert(cache_key.clone());
        let cached = read_cached_string(cache_db, &cache_key)?;
        let output = archive_year_path(html_root, *year);

        let mut needs_render = matches!(mode, BuildMode::Full);
        if !needs_render {
            match cached.as_deref() {
                Some(existing) if existing == digest => {
                    if !output.exists() {
                        needs_render = true;
                    }
                }
                _ => needs_render = true,
            }
        }

        if needs_render {
            let scope = format!("rendering year archive {year:04}");
            let rendered = render_template_with_scope(
                &year_template,
                minijinja::context! { year => year, posts => summaries },
                &scope,
            )?;

            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&output, rendered)
                .with_context(|| format!("failed to write {}", output.display()))?;
            store_cached_string(cache_db, &cache_key, &digest)?;
            log_status(verbose, "ARCHIVE", format!("Rendered year {year:04}"));
        } else {
            log_status(verbose, "ARCHIVE", format!("Year {year:04} unchanged"));
        }
    }

    let mut month_keys: BTreeSet<String> = BTreeSet::new();
    for ((year, month), group) in month_groups.iter().rev() {
        let summaries = group
            .iter()
            .map(|post| build_post_summary(config, post))
            .collect::<Result<Vec<_>>>()?;
        let payload = MonthArchiveCachePayload {
            year: *year,
            month: *month,
            posts: &summaries,
        };
        let digest = compute_cache_digest(&payload)?;
        let cache_key = format!("{MONTH_ARCHIVE_PREFIX}{year:04}-{month:02}");
        month_keys.insert(cache_key.clone());
        let cached = read_cached_string(cache_db, &cache_key)?;

        let output = archive_month_path(html_root, *year, *month);

        let mut needs_render = matches!(mode, BuildMode::Full);
        if !needs_render {
            match cached.as_deref() {
                Some(existing) if existing == digest.as_str() => {
                    if !output.exists() {
                        needs_render = true;
                    }
                }
                _ => needs_render = true,
            }
        }

        if needs_render {
            let scope = format!("rendering month archive {year:04}-{month:02}");
            let rendered = render_template_with_scope(
                &month_template,
                minijinja::context! { year => year, month => month, posts => summaries },
                &scope,
            )?;

            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&output, rendered)
                .with_context(|| format!("failed to write {}", output.display()))?;
            store_cached_string(cache_db, &cache_key, &digest)?;
            log_status(
                verbose,
                "ARCHIVE",
                format!("Rendered month {year:04}-{month:02}"),
            );
        } else {
            log_status(
                verbose,
                "ARCHIVE",
                format!("Month {year:04}-{month:02} unchanged"),
            );
        }
    }

    cleanup_month_archives(cache_db, html_root, &month_keys)?;
    cleanup_year_archives(cache_db, html_root, &year_keys)?;

    Ok(())
}

pub(super) fn render_tag_archives(
    posts: &[Post],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache_db: &sled::Db,
    mode: BuildMode,
    verbose: bool,
) -> Result<()> {
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
        let keep_keys = BTreeSet::new();
        cleanup_tag_cache(cache_db, html_root, &keep_keys)?;
        return Ok(());
    }

    let mut plans = Vec::new();
    for bucket in buckets.values() {
        let summaries = bucket
            .indices
            .iter()
            .map(|&idx| build_post_summary(config, &posts[idx]))
            .collect::<Result<Vec<_>>>()?;
        let pagination = PaginationContext {
            current: 1,
            total: 1,
            prev: String::new(),
            next: String::new(),
        };
        plans.push(TagPagePlan {
            tag: bucket.name.clone(),
            slug: bucket.slug.clone(),
            summaries,
            pagination,
            output: tag_index_path(html_root, &bucket.slug),
        });
    }

    let mut keep_keys: BTreeSet<String> = BTreeSet::new();

    for plan in plans {
        let cache_key = format!("{TAG_CACHE_PREFIX}{}", plan.slug);
        keep_keys.insert(cache_key.clone());

        let payload = TagCachePayload {
            tag: &plan.tag,
            posts: &plan.summaries,
            pagination: &plan.pagination,
        };
        let digest = compute_cache_digest(&payload)
            .with_context(|| format!("failed to compute digest for tag {}", plan.slug))?;
        let cached = read_cached_string(cache_db, &cache_key)?;

        let mut needs_render = matches!(mode, BuildMode::Full);
        if !needs_render {
            match cached.as_deref() {
                Some(existing) if existing == digest.as_str() => {
                    if !plan.output.exists() {
                        needs_render = true;
                    }
                }
                _ => needs_render = true,
            }
        }

        let slug = plan.slug.clone();

        if needs_render {
            render_tag_page(&tag_template, plan)?;
            store_cached_string(cache_db, &cache_key, &digest)?;
            log_status(verbose, "TAG", format!("Rendered tag {}", slug));
        } else {
            log_status(verbose, "TAG", format!("Tag {} unchanged", slug));
        }
    }

    cleanup_tag_cache(cache_db, html_root, &keep_keys)?;

    Ok(())
}

pub(super) fn page_url(page_number: usize) -> String {
    format!("/page/{}/", page_number)
}

pub(super) fn tag_slug(tag: &str) -> String {
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

pub(super) fn tag_index_url(slug: &str) -> String {
    format!("/tags/{}/", slug)
}

pub(super) fn page_output_path(html_root: &Path, page_number: usize) -> PathBuf {
    html_root
        .join("page")
        .join(page_number.to_string())
        .join("index.html")
}

pub(super) fn tag_index_path(html_root: &Path, slug: &str) -> PathBuf {
    html_root.join("tags").join(slug).join("index.html")
}

pub(super) fn archive_year_path(html_root: &Path, year: i32) -> PathBuf {
    html_root.join(format!("{:04}", year)).join("index.html")
}

pub(super) fn archive_month_path(html_root: &Path, year: i32, month: u8) -> PathBuf {
    html_root
        .join(format!("{:04}", year))
        .join(format!("{:02}", month))
        .join("index.html")
}

fn render_tag_page(template: &minijinja::Template<'_, '_>, plan: TagPagePlan) -> Result<()> {
    let scope = format!("rendering tag page for '{}'", plan.tag);
    let rendered = render_template_with_scope(
        template,
        minijinja::context! { tag => plan.tag, posts => plan.summaries, pagination => plan.pagination },
        &scope,
    )?;

    if let Some(parent) = plan.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&plan.output, &rendered)
        .with_context(|| format!("failed to write {}", plan.output.display()))?;
    Ok(())
}

fn render_page(template: &minijinja::Template<'_, '_>, plan: PagePlan) -> Result<()> {
    let scope = format!(
        "rendering homepage page {} of {}",
        plan.pagination.current, plan.pagination.total
    );
    let rendered = render_template_with_scope(
        template,
        minijinja::context! { posts => plan.summaries, pagination => plan.pagination },
        &scope,
    )?;

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

fn cleanup_tag_cache(db: &sled::Db, html_root: &Path, keep: &BTreeSet<String>) -> Result<()> {
    let mut stale: Vec<String> = Vec::new();
    for entry in db.scan_prefix(TAG_CACHE_PREFIX.as_bytes()) {
        let (key, _) = entry.context("failed to iterate tag cache entries")?;
        let key_vec = key.to_vec();
        let key_str =
            String::from_utf8(key_vec.clone()).context("tag cache key is not valid utf-8")?;
        if !keep.contains(&key_str) {
            stale.push(key_str);
        }
    }

    for key in stale {
        db.remove(key.as_bytes())
            .context("failed to remove stale tag cache entry")?;
        if let Some(slug) = key.strip_prefix(TAG_CACHE_PREFIX) {
            if slug.is_empty() {
                continue;
            }
            let output = tag_index_path(html_root, slug);
            remove_file_if_exists(&output)?;
            if let Some(parent) = output.parent() {
                remove_dir_if_empty(parent)?;
            }
        }
    }

    Ok(())
}

fn cleanup_month_archives(db: &sled::Db, html_root: &Path, keep: &BTreeSet<String>) -> Result<()> {
    let mut stale: Vec<String> = Vec::new();
    for entry in db.scan_prefix(MONTH_ARCHIVE_PREFIX.as_bytes()) {
        let (key, _) = entry.context("failed to iterate month archive cache entries")?;
        let key_vec = key.to_vec();
        let key_str = String::from_utf8(key_vec.clone())
            .context("month archive cache key is not valid utf-8")?;
        if !keep.contains(&key_str) {
            stale.push(key_str);
        }
    }

    for key in stale {
        db.remove(key.as_bytes())
            .context("failed to remove stale month archive cache entry")?;
        if let Some(suffix) = key.strip_prefix(MONTH_ARCHIVE_PREFIX)
            && let Some((year_str, month_str)) = suffix.split_once('-')
            && let (Ok(year), Ok(month)) = (year_str.parse::<i32>(), month_str.parse::<u8>())
        {
            let output = archive_month_path(html_root, year, month);
            remove_file_if_exists(&output)?;
            if let Some(parent) = output.parent() {
                remove_dir_if_empty(parent)?;
            }
        }
    }

    Ok(())
}

fn cleanup_year_archives(db: &sled::Db, html_root: &Path, keep: &BTreeSet<String>) -> Result<()> {
    let mut stale: Vec<String> = Vec::new();
    for entry in db.scan_prefix(YEAR_ARCHIVE_PREFIX.as_bytes()) {
        let (key, _) = entry.context("failed to iterate year archive cache entries")?;
        let key_vec = key.to_vec();
        let key_str = String::from_utf8(key_vec.clone())
            .context("year archive cache key is not valid utf-8")?;
        if !keep.contains(&key_str) {
            stale.push(key_str);
        }
    }

    for key in stale {
        db.remove(key.as_bytes())
            .context("failed to remove stale year archive cache entry")?;
        if let Some(year_str) = key.strip_prefix(YEAR_ARCHIVE_PREFIX)
            && let Ok(year) = year_str.parse::<i32>()
        {
            let output = archive_year_path(html_root, year);
            remove_file_if_exists(&output)?;
            if let Some(parent) = output.parent() {
                remove_dir_if_empty(parent)?;
            }
        }
    }

    Ok(())
}

fn cleanup_homepage_pages(html_root: &Path, keep: &[StoredPage]) -> Result<()> {
    let page_dir = html_root.join("page");
    if !page_dir.exists() {
        return Ok(());
    }

    // Build set of page numbers we want to keep (skip homepage which is page_number=0)
    let keep_pages: HashSet<usize> = keep
        .iter()
        .filter(|p| p.page_number > 0)
        .map(|p| p.page_number)
        .collect();

    // Read all subdirectories in html/page
    let entries = fs::read_dir(&page_dir)
        .with_context(|| format!("failed to read directory {}", page_dir.display()))?;

    for entry in entries {
        let entry = entry.context("failed to read directory entry")?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && let Ok(page_num) = name.parse::<usize>()
            && !keep_pages.contains(&page_num)
        {
            // This is a stale page directory, remove it
            fs::remove_dir_all(&path).with_context(|| {
                format!("failed to remove stale page directory {}", path.display())
            })?;
        }
    }

    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredPage {
    page_number: usize, // 0 = homepage, 1+ = numbered pages
    posts: Vec<String>,
}

struct TagBucket {
    name: String,
    slug: String,
    indices: Vec<usize>,
}

#[derive(Serialize)]
struct PaginationContext {
    current: usize,
    total: usize,
    prev: String,
    next: String,
}

#[derive(Serialize)]
struct TagCachePayload<'a> {
    tag: &'a str,
    posts: &'a [PostSummary],
    pagination: &'a PaginationContext,
}

#[derive(Serialize)]
struct YearArchiveCachePayload<'a> {
    year: i32,
    posts: &'a [PostSummary],
}

#[derive(Serialize)]
struct MonthArchiveCachePayload<'a> {
    year: i32,
    month: u8,
    posts: &'a [PostSummary],
}

struct TagPagePlan {
    tag: String,
    slug: String,
    summaries: Vec<PostSummary>,
    pagination: PaginationContext,
    output: PathBuf,
}

struct PagePlan {
    summaries: Vec<PostSummary>,
    pagination: PaginationContext,
    outputs: Vec<PathBuf>,
}
