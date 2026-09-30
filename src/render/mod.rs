mod assets;
mod cache;
mod feeds;
mod listing;
mod outputs;
mod pages;
mod posts;
mod templates;
mod utils;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use blake3::Hasher;

use crate::config::Config;
use crate::content::{Post, discover_posts};
use crate::search;
use crate::template;

use assets::{compute_static_digest, copy_static_assets, static_outputs};
use cache::{open_cache_db, read_cached_string, store_cached_string};
use feeds::{feed_outputs, render_feeds};
use listing::{
    ArchiveYear, HomePageCache, build_archive_years, index_outputs, render_archives,
    render_homepage, render_tag_archives,
};
use outputs::{
    Inventory, OUTPUTS_KEY, OutputManifest, OutputRun, PENDING_OUTPUTS_KEY, Producer,
    legacy_manifest, relative_output,
};
use pages::{page_outputs, render_pages};
use posts::{build_post_summary, post_outputs, render_posts};
use templates::load_templates;
use utils::log_status;

pub(super) const CACHE_DIR: &str = ".bckt/cache";
pub(super) const HOME_PAGES_KEY: &str = "home_pages";
pub(super) const POST_HASH_PREFIX: &str = "post:";
pub(super) const TAG_CACHE_PREFIX: &str = "tag_index:";
pub(super) const YEAR_ARCHIVE_PREFIX: &str = "archive_year:";
pub(super) const MONTH_ARCHIVE_PREFIX: &str = "archive_month:";
pub(super) const FEED_CACHE_PREFIX: &str = "feed:";
pub(super) const SITEMAP_CACHE_KEY: &str = "sitemap";
const SITE_INPUTS_KEY: &str = "site_inputs_hash";
const STATIC_HASH_KEY: &str = "static_hash";
pub(super) const SEARCH_INDEX_KEY: &str = "search_index_hash";

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

#[derive(Default, Debug)]
struct RenderStats {
    posts_rendered: usize,
    posts_skipped: usize,
    pages_rendered: usize,
    search_documents: usize,
    static_assets_copied: usize,
}

pub fn render_site(root: &Path, plan: RenderPlan) -> Result<()> {
    let started = Instant::now();
    let mut stats = RenderStats::default();
    let config_path = root.join("bckt.yaml");
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

    if plan.verbose {
        if plan.mode == BuildMode::Full {
            log_status(true, "MODE", "Full rebuild requested");
        } else {
            log_status(true, "MODE", "Incremental rebuild requested");
        }
    }

    let cache = HomePageCache::new(cache_db.clone());
    let search_path = search::resolve_asset_path(&html_root, &config.search.asset_path);
    // Capture ownership before any stage prunes the legacy cache keys.
    let committed = match OutputManifest::load(&cache_db, OUTPUTS_KEY)? {
        Some(manifest) => manifest,
        None => legacy_manifest(&cache_db, &html_root, &cache, &search_path)?,
    };
    let pending = OutputManifest::load(&cache_db, PENDING_OUTPUTS_KEY)?;

    // Discover and sort all posts upfront so we can build the archive_years global
    // before any template is rendered (including individual post pages).
    let posts_dir = root.join("posts");
    let mut all_posts = if posts_dir.exists() {
        discover_posts(&posts_dir, &config, Some(&cache_db))?
    } else {
        Vec::new()
    };
    all_posts.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.slug.cmp(&b.slug)));
    let archive_years = build_archive_years(&all_posts);
    env.add_global(
        "archive_years",
        minijinja::value::Value::from_serialize(&archive_years),
    );

    let site_inputs_hash = compute_site_inputs_hash(&config_raw, &template_hash, &archive_years)?;
    let stored_site_hash = read_cached_string(&cache_db, SITE_INPUTS_KEY)?;
    let site_changed = stored_site_hash.as_deref() != Some(site_inputs_hash.as_str());

    let current = current_inventories(root, &html_root, &config, &all_posts, &search_path, plan)?;
    let outputs = OutputRun::plan(committed, pending, current);
    let effective_mode = resolve_mode(plan, site_changed, outputs.requires_refresh());

    if plan.verbose {
        match effective_mode {
            BuildMode::Full => log_status(true, "MODE", "Executing full rebuild"),
            BuildMode::Changed => log_status(true, "MODE", "Executing incremental rebuild"),
        }
    }

    outputs.begin(&cache_db)?;

    let posts = if plan.posts {
        log_status(plan.verbose, "STEP", "Rendering posts");
        let (rendered_posts, skipped_posts) = render_posts(
            &all_posts,
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
            format!("Processed {} posts", all_posts.len()),
        );
        stats.posts_rendered = rendered_posts;
        stats.posts_skipped = skipped_posts;
        all_posts
    } else {
        log_status(plan.verbose, "STEP", "Skipping post rendering");
        Vec::new()
    };

    if plan.posts {
        log_status(plan.verbose, "STEP", "Rendering indexes and feeds");

        // Build each post's summary exactly once and share it across the
        // homepage, tag, archive, and feed renderers. Invariant: summaries[i]
        // corresponds to posts[i]; both stay sorted ascending by (date, slug),
        // so any future re-sort or filter of `posts` must carry `summaries`.
        let summaries = posts
            .iter()
            .map(|post| build_post_summary(&config, post))
            .collect::<Result<Vec<_>>>()?;

        render_homepage(
            &posts,
            &summaries,
            &html_root,
            &config,
            &env,
            &cache,
            effective_mode,
        )?;
        render_tag_archives(
            &posts,
            &summaries,
            &html_root,
            &env,
            &cache_db,
            effective_mode,
            plan.verbose,
        )?;
        render_archives(
            &posts,
            &summaries,
            &html_root,
            &env,
            &cache_db,
            effective_mode,
            plan.verbose,
        )?;
        render_feeds(
            &posts,
            &summaries,
            &html_root,
            &config,
            &env,
            &cache_db,
            effective_mode,
        )?;

        let artifact = search::build_index(&config, &posts)?;
        stats.search_documents = artifact.document_count;
        let cached_search_hash = read_cached_string(&cache_db, SEARCH_INDEX_KEY)?;
        let needs_search = matches!(effective_mode, BuildMode::Full)
            || cached_search_hash.as_deref() != Some(artifact.digest.as_str())
            || !search_path.exists();

        if needs_search {
            if let Some(parent) = search_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&search_path, &artifact.bytes).with_context(|| {
                format!("failed to write search index to {}", search_path.display())
            })?;
            log_status(
                plan.verbose,
                "SEARCH",
                format!(
                    "Updated search index ({} documents)",
                    artifact.document_count
                ),
            );
        } else {
            log_status(plan.verbose, "SEARCH", "Search index unchanged");
        }

        store_cached_string(&cache_db, SEARCH_INDEX_KEY, &artifact.digest)?;
    }

    stats.pages_rendered = render_pages(root, &html_root, &env, plan.verbose)?;

    if plan.static_assets {
        let static_hash = compute_static_digest(root)?;
        let stored_static_hash = read_cached_string(&cache_db, STATIC_HASH_KEY)?;
        let static_changed = stored_static_hash.as_deref() != Some(static_hash.as_str());
        let should_copy_static = matches!(effective_mode, BuildMode::Full)
            || static_changed
            || outputs.overlays_other_outputs(Producer::Static);
        if should_copy_static {
            log_status(plan.verbose, "STATIC", "Copying static assets");
            stats.static_assets_copied = copy_static_assets(root, &html_root)?;
        } else {
            log_status(plan.verbose, "STATIC", "Static assets unchanged");
            stats.static_assets_copied = 0;
        }
        store_cached_string(&cache_db, STATIC_HASH_KEY, &static_hash)?;
    } else {
        log_status(plan.verbose, "STATIC", "Skipping static assets");
        stats.static_assets_copied = 0;
    }

    let removed = outputs.finish(&cache_db, &html_root)?;
    // Only a successful post build may consume a site-input change; partial
    // runs leave it pending for the next post build.
    if plan.posts {
        store_cached_string(&cache_db, SITE_INPUTS_KEY, &site_inputs_hash)?;
    }
    if removed > 0 {
        log_status(
            plan.verbose,
            "CLEAN",
            format!("Removed {removed} obsolete generated files"),
        );
    }

    cache_db.flush().context("failed to flush cache database")?;

    log_status(plan.verbose, "DONE", "Render complete");

    let total_posts = stats.posts_rendered + stats.posts_skipped;
    let elapsed = started.elapsed();
    println!(
        "[SUMMARY] posts rendered: {}/{} (skipped {}); pages: {}; search docs: {}; static assets copied: {}; elapsed: {:.2?}",
        stats.posts_rendered,
        total_posts,
        stats.posts_skipped,
        stats.pages_rendered,
        stats.search_documents,
        stats.static_assets_copied,
        elapsed
    );

    Ok(())
}

/// Digest of inputs every rendered page can observe: config, templates, and
/// the `archive_years` global.
fn compute_site_inputs_hash(
    config_raw: &str,
    template_hash: &str,
    archive_years: &[ArchiveYear],
) -> Result<String> {
    let mut hasher = Hasher::new();
    hasher.update(b"bckt-site-inputs-v2\0");
    hasher.update(config_raw.as_bytes());
    hasher.update(b"\0");
    hasher.update(template_hash.as_bytes());
    hasher.update(b"\0");
    let years = serde_json::to_vec(archive_years).context("failed to serialize archive_years")?;
    hasher.update(&years);
    Ok(hasher.finalize().to_hex().to_string())
}

fn resolve_mode(plan: RenderPlan, site_changed: bool, refresh_outputs: bool) -> BuildMode {
    if plan.mode == BuildMode::Full {
        return BuildMode::Full;
    }
    if site_changed {
        log_status(
            plan.verbose,
            "MODE",
            "Config or templates changed; forcing full rebuild",
        );
        return BuildMode::Full;
    }
    if refresh_outputs {
        log_status(
            plan.verbose,
            "MODE",
            "Output ownership changed or a previous render failed; forcing full rebuild",
        );
        return BuildMode::Full;
    }
    BuildMode::Changed
}

/// Output inventories for the producers this plan runs. Pages always run.
fn current_inventories(
    root: &Path,
    html_root: &Path,
    config: &Config,
    posts: &[Post],
    search_path: &Path,
    plan: RenderPlan,
) -> Result<BTreeMap<Producer, Inventory>> {
    let mut current = BTreeMap::new();
    if plan.posts {
        current.insert(Producer::Posts, post_outputs(posts, html_root));
        current.insert(Producer::Indexes, index_outputs(posts, config, html_root));
        current.insert(Producer::Feeds, feed_outputs(config, html_root));
        let search = relative_output(html_root, search_path)
            .into_iter()
            .collect();
        current.insert(Producer::Search, search);
    }
    current.insert(Producer::Pages, page_outputs(root, html_root)?);
    if plan.static_assets {
        current.insert(Producer::Static, static_outputs(root, html_root)?);
    }
    Ok(current)
}
