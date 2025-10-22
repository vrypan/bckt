use std::collections::{BTreeMap, HashSet};
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
use crate::utils::absolute_url;

use super::listing::{page_url, tag_index_url, tag_slug};
use super::posts::{PostSummary, att_to_absolute, build_post_summary};
use super::templates::render_template_with_scope;
use super::utils::{format_rfc2822, format_rfc3339, sanitize_cdata, xml_escape};

pub(super) fn render_feeds(
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
            .rev()
            .filter(|post| post.tags.iter().any(|t| t.eq(&tag)))
            .collect();
        let output_path = html_root.join(format!("rss-{}.xml", slug));
        let title = config.title.clone().unwrap_or_else(|| "bckt".to_string());
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
    // Posts are sorted ascending, but RSS feeds should show newest first
    let posts_ref: Vec<&Post> = posts.iter().rev().collect();
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
    let resolved_title =
        title.unwrap_or_else(|| config.title.clone().unwrap_or_else(|| "bckt".to_string()));
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

    let scope = format!("rendering feed {}", feed_path);
    let rendered =
        render_template_with_scope(&template, minijinja::context! { feed => context }, &scope)?;

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

    // Posts are sorted ASCENDING (oldest first, newest last)
    // Calculate pagination structure (must match listing.rs logic)
    let remainder = posts.len() % per_page;
    let home_page_size = if posts.len() < per_page {
        posts.len()
    } else if remainder == 0 {
        per_page
    } else if remainder < per_page {
        remainder + per_page
    } else {
        per_page
    };
    let regular_page_count = (posts.len() - home_page_size) / per_page;

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
        // The newest post on this page is at end-1 (since sorted ascending)
        let page_date = format_rfc3339(&posts[end - 1].date)?;
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
    fs::write(&output_path, buffer)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
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
        let first = &posts[bucket.indices[0]];
        entries.push(SitemapEntry {
            loc: absolute_url(&config.base_url, &tag_index_url(&bucket.slug)),
            lastmod: Some(format_rfc3339(&first.date)?),
        });
    }

    Ok(entries)
}

fn build_feed_item(config: &Config, post: &Post) -> Result<PostSummary> {
    let mut summary = build_post_summary(config, post)?;

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
