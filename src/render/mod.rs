mod assets;
mod cache;
mod feeds;
mod listing;
mod pages;
mod posts;
mod templates;
mod utils;

#[cfg(test)]
mod tests;

use std::fs;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use blake3::Hasher;

use crate::config::Config;
use crate::search;
use crate::template;

use assets::{
    ThemeAssetCopy, compute_static_digest, compute_theme_asset_digest, copy_static_assets,
    copy_theme_assets,
};
use cache::{open_cache_db, read_cached_string, store_cached_string};
use feeds::render_feeds;
use listing::{HomePageCache, render_archives, render_homepage, render_tag_archives};
use pages::render_pages;
use posts::render_posts;
use templates::load_templates;
use utils::log_status;

pub(super) const CACHE_DIR: &str = ".bckt/cache";
pub(super) const HOME_PAGES_KEY: &str = "home_pages";
pub(super) const POST_HASH_PREFIX: &str = "post:";
pub(super) const TAG_CACHE_PREFIX: &str = "tag_index:";
pub(super) const YEAR_ARCHIVE_PREFIX: &str = "archive_year:";
pub(super) const MONTH_ARCHIVE_PREFIX: &str = "archive_month:";
const SITE_INPUTS_KEY: &str = "site_inputs_hash";
const STATIC_HASH_KEY: &str = "static_hash";
const SEARCH_INDEX_KEY: &str = "search_index_hash";
const THEME_ASSET_HASH_KEY: &str = "theme_asset_hash";

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
    theme_assets_copied: usize,
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

    let posts = if plan.posts {
        log_status(plan.verbose, "STEP", "Rendering posts");
        let (posts, rendered_posts, skipped_posts) = render_posts(
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
        stats.posts_rendered = rendered_posts;
        stats.posts_skipped = skipped_posts;
        posts
    } else {
        log_status(plan.verbose, "STEP", "Skipping post rendering");
        Vec::new()
    };

    if plan.posts {
        log_status(plan.verbose, "STEP", "Rendering indexes and feeds");
        render_homepage(&posts, &html_root, &config, &env, &cache)?;
        render_tag_archives(
            &posts,
            &html_root,
            &config,
            &env,
            &cache_db,
            effective_mode,
            plan.verbose,
        )?;
        render_archives(
            &posts,
            &html_root,
            &config,
            &env,
            &cache_db,
            effective_mode,
            plan.verbose,
        )?;
        render_feeds(&posts, &html_root, &config, &env)?;

        let artifact = search::build_index(&config, &posts)?;
        stats.search_documents = artifact.document_count;
        let search_path = search::resolve_asset_path(&html_root, &config.search.asset_path);
        let cached_search_hash = read_cached_string(&cache_db, SEARCH_INDEX_KEY)?;
        let needs_search = cached_search_hash.as_deref() != Some(artifact.digest.as_str())
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
        store_cached_string(&cache_db, SITE_INPUTS_KEY, &site_inputs_hash)?;
    }

    stats.pages_rendered = render_pages(root, &html_root, &env, plan.verbose)?;

    if plan.static_assets {
        let static_hash = compute_static_digest(root)?;
        let stored_static_hash = read_cached_string(&cache_db, STATIC_HASH_KEY)?;
        let static_changed = stored_static_hash.as_deref() != Some(static_hash.as_str());
        let should_copy_static = matches!(effective_mode, BuildMode::Full) || static_changed;
        if should_copy_static {
            log_status(plan.verbose, "STATIC", "Copying static assets");
            stats.static_assets_copied = copy_static_assets(root, &html_root)?;
        } else {
            log_status(plan.verbose, "STATIC", "Static assets unchanged");
            stats.static_assets_copied = 0;
        }
        store_cached_string(&cache_db, STATIC_HASH_KEY, &static_hash)?;

        if let Some(theme_name) = config.theme.as_deref() {
            let theme_hash = compute_theme_asset_digest(root, theme_name)?;
            let stored_theme_hash = read_cached_string(&cache_db, THEME_ASSET_HASH_KEY)?;
            let theme_changed = stored_theme_hash.as_deref() != Some(theme_hash.as_str());
            let should_copy_theme = matches!(effective_mode, BuildMode::Full) || theme_changed;

            if should_copy_theme {
                match copy_theme_assets(root, &html_root, theme_name)? {
                    ThemeAssetCopy::Copied(count) => {
                        stats.theme_assets_copied = count;
                        log_status(
                            plan.verbose,
                            "THEME",
                            format!("Copied {count} theme asset(s) for {theme_name}"),
                        );
                    }
                    ThemeAssetCopy::SkippedMissing => {
                        stats.theme_assets_copied = 0;
                        log_status(
                            plan.verbose,
                            "THEME",
                            format!("Theme {theme_name} has no assets directory"),
                        );
                    }
                }
            } else {
                stats.theme_assets_copied = 0;
                log_status(plan.verbose, "THEME", "Theme assets unchanged");
            }

            store_cached_string(&cache_db, THEME_ASSET_HASH_KEY, &theme_hash)?;
        }
    } else {
        log_status(plan.verbose, "STATIC", "Skipping static assets");
        stats.static_assets_copied = 0;
        stats.theme_assets_copied = 0;
    }

    cache_db.flush().context("failed to flush cache database")?;

    log_status(plan.verbose, "DONE", "Render complete");

    let total_posts = stats.posts_rendered + stats.posts_skipped;
    let elapsed = started.elapsed();
    println!(
        "[SUMMARY] posts rendered: {}/{} (skipped {}); pages: {}; search docs: {}; static assets copied: {}; theme assets copied: {}; elapsed: {:.2?}",
        stats.posts_rendered,
        total_posts,
        stats.posts_skipped,
        stats.pages_rendered,
        stats.search_documents,
        stats.static_assets_copied,
        stats.theme_assets_copied,
        elapsed
    );

    Ok(())
}

fn compute_site_inputs_hash(config_raw: &str, template_hash: &str) -> String {
    let mut hasher = Hasher::new();
    hasher.update(config_raw.as_bytes());
    hasher.update(template_hash.as_bytes());
    hasher.finalize().to_hex().to_string()
}
