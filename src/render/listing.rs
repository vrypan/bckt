use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use minijinja::Environment;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::content::Post;

use super::cache::{read_cached_string, store_cached_string};
use super::outputs::{Inventory, relative_output};
use super::posts::{PostSummary, post_key};
use super::templates::render_template_with_scope;
use super::utils::{compute_cache_digest, compute_pagination_layout, log_status};
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
    summaries: &[PostSummary],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache: &HomePageCache,
    mode: BuildMode,
) -> Result<()> {
    let template = env
        .get_template("index.html")
        .context("index.html template missing")?;

    let layout = compute_pagination_layout(posts.len(), config.homepage_posts);
    let per_page = layout.per_page;
    let regular_page_count = layout.regular_page_count;
    let home_start = regular_page_count * per_page;

    let stored_pages = cache.load_pages()?;
    let stored_map: HashMap<usize, &StoredPage> = stored_pages
        .iter()
        .map(|page| (page.page_number, page))
        .collect();

    // Regular pages 1..=N hold the oldest posts; page 0 (the homepage) holds
    // the newest. Each page lists its posts newest first.
    let mut new_records = Vec::new();
    let mut plans: Vec<PagePlan> = Vec::new();
    for page_num in (1..=regular_page_count).chain([0]) {
        let range = if page_num == 0 {
            home_start..posts.len()
        } else {
            (page_num - 1) * per_page..page_num * per_page
        };
        let page_refs: Vec<&PostSummary> = summaries[range.clone()].iter().rev().collect();
        let pagination = homepage_pagination(page_num, regular_page_count);
        let content_digest = compute_cache_digest(&HomePageCachePayload {
            posts: &page_refs,
            pagination: &pagination,
        })?;
        let record = StoredPage {
            page_number: page_num,
            posts: posts[range].iter().rev().map(post_key).collect(),
            content_digest,
        };
        let output = if page_num == 0 {
            html_root.join("index.html")
        } else {
            page_output_path(html_root, page_num)
        };

        let unchanged = stored_map.get(&page_num).is_some_and(|cached| {
            cached.posts == record.posts && cached.content_digest == record.content_digest
        });
        if matches!(mode, BuildMode::Full) || !unchanged || !output.exists() {
            plans.push(PagePlan {
                summaries: page_refs,
                pagination,
                outputs: vec![output],
            });
        }
        new_records.push(record);
    }

    for plan in plans {
        render_page(&template, plan)?;
    }

    cache.store_pages(&new_records)?;

    Ok(())
}

/// Notebook pagination: page 1 is the oldest page, the homepage (page 0) is the
/// newest and reports itself as the last page.
fn homepage_pagination(page_num: usize, regular_page_count: usize) -> PaginationContext {
    let total = regular_page_count + 1;
    let newer = |page: usize| {
        if page < regular_page_count {
            page_url(page + 1)
        } else {
            "/".to_string()
        }
    };
    let (current, prev, next) = match page_num {
        0 if regular_page_count > 0 => (total, page_url(regular_page_count), String::new()),
        0 => (total, String::new(), String::new()),
        1 => (1, String::new(), newer(1)),
        page => (page, page_url(page - 1), newer(page)),
    };
    PaginationContext {
        current,
        total,
        prev,
        next,
    }
}

pub(super) fn render_archives(
    posts: &[Post],
    summaries: &[PostSummary],
    html_root: &Path,
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

    let (year_groups, month_groups) = group_archives(posts);

    let mut year_keys: BTreeSet<String> = BTreeSet::new();
    for (year, group) in year_groups.iter().rev() {
        let summaries: Vec<&PostSummary> = group.iter().rev().map(|&idx| &summaries[idx]).collect();
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
        let summaries: Vec<&PostSummary> = group.iter().rev().map(|&idx| &summaries[idx]).collect();
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

    cleanup_cache_entries(cache_db, MONTH_ARCHIVE_PREFIX, &month_keys)?;
    cleanup_cache_entries(cache_db, YEAR_ARCHIVE_PREFIX, &year_keys)?;

    Ok(())
}

pub(super) fn render_tag_archives(
    posts: &[Post],
    summaries: &[PostSummary],
    html_root: &Path,
    env: &Environment<'static>,
    cache_db: &sled::Db,
    mode: BuildMode,
    verbose: bool,
) -> Result<()> {
    let tag_template = env
        .get_template("tag.html")
        .context("tag.html template missing")?;

    let buckets = collect_tag_buckets(posts);
    if buckets.is_empty() {
        cleanup_tag_cache(cache_db, &BTreeSet::new())?;
        return Ok(());
    }

    let mut plans = Vec::new();
    for bucket in buckets.values() {
        let summaries: Vec<&PostSummary> = bucket
            .indices
            .iter()
            .rev()
            .map(|&idx| &summaries[idx])
            .collect();
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

    cleanup_tag_cache(cache_db, &keep_keys)?;

    Ok(())
}

type ArchiveGroups = (BTreeMap<i32, Vec<usize>>, BTreeMap<(i32, u8), Vec<usize>>);

/// Group post indices so each summary is referenced (not rebuilt) per group.
fn group_archives(posts: &[Post]) -> ArchiveGroups {
    let mut year_groups: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    let mut month_groups: BTreeMap<(i32, u8), Vec<usize>> = BTreeMap::new();
    for (idx, post) in posts.iter().enumerate() {
        year_groups.entry(post.date.year()).or_default().push(idx);
        month_groups
            .entry((post.date.year(), post.date.month() as u8))
            .or_default()
            .push(idx);
    }
    (year_groups, month_groups)
}

fn collect_tag_buckets(posts: &[Post]) -> BTreeMap<String, TagBucket> {
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
    buckets
}

/// Every file the homepage, tag, and archive renderers generate, cached or not.
pub(super) fn index_outputs(posts: &[Post], config: &Config, html_root: &Path) -> Inventory {
    let layout = compute_pagination_layout(posts.len(), config.homepage_posts);
    let mut files = vec![html_root.join("index.html")];
    files.extend((1..=layout.regular_page_count).map(|page| page_output_path(html_root, page)));
    files.extend(
        collect_tag_buckets(posts)
            .keys()
            .map(|slug| tag_index_path(html_root, slug)),
    );
    let (year_groups, month_groups) = group_archives(posts);
    files.extend(
        year_groups
            .keys()
            .map(|year| archive_year_path(html_root, *year)),
    );
    files.extend(
        month_groups
            .keys()
            .map(|(year, month)| archive_month_path(html_root, *year, *month)),
    );
    files
        .iter()
        .filter_map(|file| relative_output(html_root, file))
        .collect()
}

/// Output files recorded by the homepage cache before ownership tracking.
pub(super) fn legacy_homepage_outputs(
    cache: &HomePageCache,
    html_root: &Path,
) -> Result<Inventory> {
    let files = cache.load_pages()?.into_iter().map(|page| {
        if page.page_number == 0 {
            html_root.join("index.html")
        } else {
            page_output_path(html_root, page.page_number)
        }
    });
    Ok(files
        .filter_map(|file| relative_output(html_root, &file))
        .collect())
}

pub(super) fn build_archive_years(posts: &[Post]) -> Vec<ArchiveYear> {
    let mut year_counts: BTreeMap<i32, usize> = BTreeMap::new();
    for post in posts {
        *year_counts.entry(post.date.year()).or_insert(0) += 1;
    }
    year_counts
        .iter()
        .rev()
        .map(|(&year, &count)| ArchiveYear { year, count })
        .collect()
}

pub(super) fn page_url(page_number: usize) -> String {
    format!("/page/{}/", page_number)
}

pub(super) fn tag_slug(tag: &str) -> String {
    let slug = crate::utils::slugify(tag);
    if slug.is_empty() {
        let hash = blake3::hash(tag.as_bytes());
        format!("tag-{}", &hash.to_hex().to_string()[..8])
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

/// Prune cache keys under `prefix` that are not in `keep`. Their output files
/// are removed later through output ownership, not here.
fn cleanup_cache_entries(db: &sled::Db, prefix: &str, keep: &BTreeSet<String>) -> Result<()> {
    let mut stale: Vec<String> = Vec::new();
    for entry in db.scan_prefix(prefix.as_bytes()) {
        let (key, _) = entry.context("failed to iterate cache entries")?;
        let key_str = String::from_utf8(key.to_vec()).context("cache key is not valid utf-8")?;
        if !keep.contains(&key_str) {
            stale.push(key_str);
        }
    }

    for key in stale {
        db.remove(key.as_bytes())
            .context("failed to remove stale cache entry")?;
    }

    Ok(())
}

fn cleanup_tag_cache(db: &sled::Db, keep: &BTreeSet<String>) -> Result<()> {
    cleanup_cache_entries(db, TAG_CACHE_PREFIX, keep)
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredPage {
    page_number: usize, // 0 = homepage, 1+ = numbered pages
    posts: Vec<String>,
    #[serde(default)]
    content_digest: String,
}

#[derive(Serialize)]
pub(super) struct ArchiveYear {
    pub(super) year: i32,
    pub(super) count: usize,
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
struct HomePageCachePayload<'a> {
    posts: &'a [&'a PostSummary],
    pagination: &'a PaginationContext,
}

#[derive(Serialize)]
struct TagCachePayload<'a> {
    tag: &'a str,
    posts: &'a [&'a PostSummary],
    pagination: &'a PaginationContext,
}

#[derive(Serialize)]
struct YearArchiveCachePayload<'a> {
    year: i32,
    posts: &'a [&'a PostSummary],
}

#[derive(Serialize)]
struct MonthArchiveCachePayload<'a> {
    year: i32,
    month: u8,
    posts: &'a [&'a PostSummary],
}

struct TagPagePlan<'a> {
    tag: String,
    slug: String,
    summaries: Vec<&'a PostSummary>,
    pagination: PaginationContext,
    output: PathBuf,
}

struct PagePlan<'a> {
    summaries: Vec<&'a PostSummary>,
    pagination: PaginationContext,
    outputs: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::posts::build_post_summary;
    use tempfile::TempDir;
    use time::{Duration, OffsetDateTime};

    struct Fixture {
        _temp: TempDir,
        html_root: PathBuf,
        config: Config,
        env: Environment<'static>,
        cache: HomePageCache,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();
            let html_root = temp.path().join("html");
            let cache = HomePageCache::new(sled::open(temp.path().join("db")).unwrap());
            let mut env = Environment::new();
            env.add_template(
                "index.html",
                "{{ pagination.current }}|{{ pagination.total }}|{{ pagination.prev | safe }}|{{ pagination.next | safe }}|{% for post in posts %}{{ post.slug }} {% endfor %}",
            )
            .unwrap();
            let config = Config {
                homepage_posts: 2,
                ..Default::default()
            };
            Self {
                _temp: temp,
                html_root,
                config,
                env,
                cache,
            }
        }

        fn render(&self, count: usize) {
            let posts: Vec<Post> = (0..count).map(sample_post).collect();
            let summaries = posts
                .iter()
                .map(|post| build_post_summary(&self.config, post))
                .collect::<Result<Vec<_>>>()
                .unwrap();
            render_homepage(
                &posts,
                &summaries,
                &self.html_root,
                &self.config,
                &self.env,
                &self.cache,
                BuildMode::Changed,
            )
            .unwrap();
        }

        fn page(&self, page_num: usize) -> String {
            let path = if page_num == 0 {
                self.html_root.join("index.html")
            } else {
                page_output_path(&self.html_root, page_num)
            };
            fs::read_to_string(path).unwrap()
        }
    }

    fn sample_post(index: usize) -> Post {
        let slug = format!("post-{index}");
        Post {
            title: Some(slug.clone()),
            slug: slug.clone(),
            date: OffsetDateTime::UNIX_EPOCH + Duration::days(index as i64),
            tags: Vec::new(),
            post_type: None,
            abstract_text: None,
            attached: Vec::new(),
            body_html: format!("<p>{slug}</p>"),
            excerpt: slug.clone(),
            language: "en".to_string(),
            search_text: slug.clone(),
            source_dir: PathBuf::from("posts").join(&slug),
            content_path: PathBuf::from("posts").join(&slug).join("post.md"),
            content_hash: slug.clone(),
            permalink: format!("/{slug}/"),
            extra: Default::default(),
        }
    }

    #[test]
    fn homepage_pagination_growth_updates_previous_last_page() {
        let fixture = Fixture::new();
        fixture.render(4);
        assert_eq!(fixture.page(1), "1|2||/|post-1 post-0 ");

        fixture.render(6);

        assert_eq!(fixture.page(1), "1|3||/page/2/|post-1 post-0 ");
        assert_eq!(fixture.page(2), "2|3|/page/1/|/|post-3 post-2 ");
        assert_eq!(fixture.page(0), "3|3|/page/2/||post-5 post-4 ");
    }

    #[test]
    fn homepage_pagination_shrink_updates_links() {
        let fixture = Fixture::new();
        fixture.render(6);
        assert_eq!(fixture.page(1), "1|3||/page/2/|post-1 post-0 ");

        fixture.render(4);

        assert_eq!(fixture.page(1), "1|2||/|post-1 post-0 ");
        assert_eq!(fixture.page(0), "2|2|/page/1/||post-3 post-2 ");
    }

    #[test]
    fn homepage_pagination_missing_output_is_recreated() {
        let fixture = Fixture::new();
        fixture.render(6);
        fs::remove_file(page_output_path(&fixture.html_root, 2)).unwrap();

        fixture.render(6);

        assert_eq!(fixture.page(2), "2|3|/page/1/|/|post-3 post-2 ");
    }

    #[test]
    fn homepage_pagination_noop_render_skips_writes() {
        let fixture = Fixture::new();
        fixture.render(6);
        for page in [0, 1, 2] {
            fs::write(
                if page == 0 {
                    fixture.html_root.join("index.html")
                } else {
                    page_output_path(&fixture.html_root, page)
                },
                "sentinel",
            )
            .unwrap();
        }

        fixture.render(6);

        for page in [0, 1, 2] {
            assert_eq!(fixture.page(page), "sentinel");
        }
    }
}
