use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use minijinja::Environment;
use serde::Serialize;
use serde_json::Value as JsonValue;
use time::OffsetDateTime;

use crate::config::Config;
use crate::content::Post;
use crate::utils::{absolute_url, split_csv};

use super::cache::{read_cached_string, store_cached_string};
use super::listing::{page_url, tag_index_url, tag_slug};
use super::posts::{PostSummary, att_to_absolute};
use super::templates::render_template_with_scope;
use super::utils::{
    compute_pagination_layout, format_rfc2822, format_rfc3339, remove_file_if_exists,
    sanitize_cdata, xml_escape,
};
use super::{BuildMode, FEED_CACHE_PREFIX, SITEMAP_CACHE_KEY};

pub(super) fn render_feeds(
    posts: &[Post],
    summaries: &[PostSummary],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache_db: &sled::Db,
    mode: BuildMode,
) -> Result<()> {
    // Cache keys of the feeds produced this render, used to GC stale tag feeds.
    let mut live_keys: BTreeSet<String> = BTreeSet::new();

    render_rss(posts, summaries, html_root, config, env, cache_db, mode)?;
    live_keys.insert(format!("{FEED_CACHE_PREFIX}/rss.xml"));

    for tag in config_tag_feeds(config) {
        let slug = tag_slug(&tag);
        // summaries[i] corresponds to posts[i]; keep them paired through the filter.
        let tag_posts: Vec<(&Post, &PostSummary)> = posts
            .iter()
            .zip(summaries)
            .filter(|(post, _)| post.tags.iter().any(|t| t.eq(&tag)))
            .rev()
            .collect();
        let output_path = html_root.join(format!("rss-{}.xml", slug));
        let title = config.title.clone().unwrap_or_else(|| "bckt".to_string());
        let feed_title = format!("{} · {}", tag, title);
        let site_path = format!("/tags/{}/", slug);
        let feed_path = format!("/rss-{}.xml", slug);
        live_keys.insert(format!("{FEED_CACHE_PREFIX}{feed_path}"));
        render_feed(
            tag_posts,
            config,
            env,
            &site_path,
            &feed_path,
            &output_path,
            Some(feed_title),
            cache_db,
            mode,
        )?;
    }

    cleanup_stale_feeds(cache_db, html_root, &live_keys)?;

    render_sitemap(posts, html_root, config, cache_db, mode)?;
    Ok(())
}

fn render_rss(
    posts: &[Post],
    summaries: &[PostSummary],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache_db: &sled::Db,
    mode: BuildMode,
) -> Result<()> {
    let output_path = html_root.join("rss.xml");
    // Posts are sorted ascending, but RSS feeds should show newest first
    let posts_ref: Vec<(&Post, &PostSummary)> = posts.iter().zip(summaries).rev().collect();
    render_feed(
        posts_ref,
        config,
        env,
        "/",
        "/rss.xml",
        &output_path,
        None,
        cache_db,
        mode,
    )
}

/// Remove cache entries and files for tag feeds no longer in the config. Only
/// keys under `FEED_CACHE_PREFIX` are owned here, so nothing else is touched.
fn cleanup_stale_feeds(
    cache_db: &sled::Db,
    html_root: &Path,
    keep: &BTreeSet<String>,
) -> Result<()> {
    let mut stale: Vec<String> = Vec::new();
    for entry in cache_db.scan_prefix(FEED_CACHE_PREFIX.as_bytes()) {
        let (key, _) = entry.context("failed to iterate feed cache entries")?;
        let key_str =
            String::from_utf8(key.to_vec()).context("feed cache key is not valid utf-8")?;
        if !keep.contains(&key_str) {
            stale.push(key_str);
        }
    }
    for key in stale {
        cache_db
            .remove(key.as_bytes())
            .context("failed to remove stale feed cache entry")?;
        if let Some(suffix) = key.strip_prefix(FEED_CACHE_PREFIX) {
            let file = html_root.join(suffix.trim_start_matches('/'));
            remove_file_if_exists(&file)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_feed(
    posts: Vec<(&Post, &PostSummary)>,
    config: &Config,
    env: &Environment<'static>,
    site_path: &str,
    feed_path: &str,
    output_path: &Path,
    title: Option<String>,
    cache_db: &sled::Db,
    mode: BuildMode,
) -> Result<()> {
    let template = env
        .get_template("rss.xml")
        .context("rss.xml template missing")?;

    let site_url = absolute_url(&config.base_url, site_path);
    let feed_url = absolute_url(&config.base_url, feed_path);
    let resolved_title =
        title.unwrap_or_else(|| config.title.clone().unwrap_or_else(|| "bckt".to_string()));
    let build_date = posts
        .first()
        .map(|(post, _)| post.date)
        .unwrap_or_else(OffsetDateTime::now_utc);
    let last_build_date = format_rfc2822(&build_date)?;

    let items = posts
        .into_iter()
        .take(50)
        .map(|(post, summary)| build_feed_item(config, post, summary))
        .collect::<Result<Vec<_>>>()?;

    let context = FeedContext {
        title: xml_escape(&resolved_title),
        site_url: xml_escape(&site_url),
        feed_url: xml_escape(&feed_url),
        description: xml_escape(&resolved_title),
        updated: xml_escape(&last_build_date),
        items,
    };

    let scope = format!("rendering feed {}", feed_path);
    let rendered =
        render_template_with_scope(&template, minijinja::context! { feed => context }, &scope)?;

    // Gate the write on a digest of the rendered bytes: feeds are cheap to
    // render (post-007) but rewriting churns mtimes for rsync/feed readers.
    let digest = blake3::hash(rendered.as_bytes()).to_hex().to_string();
    let cache_key = format!("{FEED_CACHE_PREFIX}{feed_path}");
    let cached = read_cached_string(cache_db, &cache_key)?;
    let needs_write = matches!(mode, BuildMode::Full)
        || cached.as_deref() != Some(digest.as_str())
        || !output_path.exists();

    if needs_write {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(output_path, rendered)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
    }
    store_cached_string(cache_db, &cache_key, &digest)?;
    Ok(())
}

fn render_sitemap(
    posts: &[Post],
    html_root: &Path,
    config: &Config,
    cache_db: &sled::Db,
    mode: BuildMode,
) -> Result<()> {
    let layout = compute_pagination_layout(posts.len(), config.homepage_posts);
    let per_page = layout.per_page;
    let regular_page_count = layout.regular_page_count;
    let mut entries: Vec<SitemapEntry> = Vec::new();

    // Homepage entry (most recent posts = end of array)
    let homepage_date = posts
        .last()
        .map(|post| format_rfc3339(&post.date))
        .transpose()?;
    entries.push(SitemapEntry {
        loc: absolute_url(&config.base_url, "/"),
        lastmod: homepage_date,
    });

    // Regular page entries (page 1, 2, 3, ...)
    // Each page's date is the NEWEST post on that page (end of the range)
    for page_num in 1..=regular_page_count {
        let start = (page_num - 1) * per_page;
        let end = start + per_page;
        let path = page_url(page_num);
        let post = posts.get(end - 1).with_context(|| {
            format!(
                "sitemap: page {page_num} index out of range (end={end}, posts={})",
                posts.len()
            )
        })?;
        let page_date = format_rfc3339(&post.date)?;
        entries.push(SitemapEntry {
            loc: absolute_url(&config.base_url, &path),
            lastmod: Some(page_date),
        });
    }

    for post in posts {
        entries.push(SitemapEntry {
            loc: absolute_url(&config.base_url, &post.permalink),
            lastmod: Some(format_rfc3339(&post.date)?),
        });
    }

    let tag_entries = collect_tag_sitemap_entries(posts, config)?;
    entries.extend(tag_entries);

    let mut buffer = String::new();
    writeln!(buffer, r#"<?xml version="1.0" encoding="utf-8"?>"#)?;
    writeln!(
        buffer,
        r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#
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
    let digest = blake3::hash(buffer.as_bytes()).to_hex().to_string();
    let cached = read_cached_string(cache_db, SITEMAP_CACHE_KEY)?;
    let needs_write = matches!(mode, BuildMode::Full)
        || cached.as_deref() != Some(digest.as_str())
        || !output_path.exists();

    if needs_write {
        fs::write(&output_path, buffer)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
    }
    store_cached_string(cache_db, SITEMAP_CACHE_KEY, &digest)?;
    Ok(())
}

fn collect_tag_sitemap_entries(posts: &[Post], config: &Config) -> Result<Vec<SitemapEntry>> {
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
        let Some(&newest_idx) = bucket.indices.last() else {
            continue;
        };
        let newest = &posts[newest_idx];
        entries.push(SitemapEntry {
            loc: absolute_url(&config.base_url, &tag_index_url(&bucket.slug)),
            lastmod: Some(format_rfc3339(&newest.date)?),
        });
    }

    Ok(entries)
}

fn build_feed_item(config: &Config, post: &Post, summary: &PostSummary) -> Result<PostSummary> {
    let mut summary = summary.clone();

    // Reprocess body with return_absolute=true for RSS feeds and sanitize CDATA
    let body = att_to_absolute(
        &post.body_html,
        &post.permalink,
        &config.base_url,
        &post.attached,
        true,
    );
    summary.body = sanitize_cdata(&body);

    // Add RSS-specific pub_date in RFC 2822 format
    let pub_date = format_rfc2822(&post.date)?;
    summary
        .extra
        .insert("pub_date".to_string(), JsonValue::String(pub_date));

    Ok(summary)
}

fn config_tag_feeds(config: &Config) -> Vec<String> {
    let mut tags = Vec::new();
    if let Some(value) = config.extra.get("rss_tags") {
        match value {
            JsonValue::String(s) => tags.extend(split_csv(s)),
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

#[derive(Serialize)]
struct FeedContext {
    title: String,
    site_url: String,
    feed_url: String,
    description: String,
    updated: String,
    items: Vec<PostSummary>,
}

#[derive(Clone)]
struct TagBucket {
    slug: String,
    indices: Vec<usize>,
}

struct SitemapEntry {
    loc: String,
    lastmod: Option<String>,
}
