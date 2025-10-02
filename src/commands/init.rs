use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const DIRECTORIES: &[&str] = &["html", "posts", "templates", "skel", "themes"];
const CONFIG_FILE: &str = "bucket3.yaml";
const THEME_NAME: &str = "bckt3";

const DEFAULT_CONFIG: &str = r#"title: "My Bucket3 Site"
base_url: "https://example.com"
homepage_posts: 5
date_format: "[year]-[month]-[day]"
paginate_tags: true
default_timezone: "+00:00"
theme: bckt3
"#;

const THEME_MANIFEST: &str = include_str!("../../themes/bckt3/theme.yaml");
const THEME_TAILWIND_CONFIG: &str = include_str!("../../themes/bckt3/tailwind.config.js");
const THEME_STYLE_SOURCE: &str = include_str!("../../themes/bckt3/style.tailwind.css");

const THEME_BASE_TEMPLATE: &str = include_str!("../../themes/bckt3/templates/base.html");
const THEME_POST_TEMPLATE: &str = include_str!("../../themes/bckt3/templates/post.html");
const THEME_INDEX_TEMPLATE: &str = include_str!("../../themes/bckt3/templates/index.html");
const THEME_TAG_TEMPLATE: &str = include_str!("../../themes/bckt3/templates/tag.html");
const THEME_ARCHIVE_YEAR_TEMPLATE: &str =
    include_str!("../../themes/bckt3/templates/archive_year.html");
const THEME_ARCHIVE_MONTH_TEMPLATE: &str =
    include_str!("../../themes/bckt3/templates/archive_month.html");
const THEME_RSS_TEMPLATE: &str = include_str!("../../themes/bckt3/templates/rss.xml");
const THEME_STYLE_CSS: &str = include_str!("../../themes/bckt3/skel/style.css");

const SAMPLE_POST: &str = r#"---
title: "Hello From bucket3rs"
slug: "hello-from-bucket3rs"
date: "2024-01-01T00:00:00Z"
tags:
  - welcome
abstract: "Kick the tires on the generator."
attached: []
images: []
---

This is the starter post. Edit it or drop in your own content to get going.
"#;

pub fn run_init_command() -> Result<()> {
    let root = env::current_dir().context("failed to resolve current directory")?;

    establish_directories(&root)?;
    seed_configuration(&root)?;
    seed_theme(&root)?;
    seed_templates(&root)?;
    seed_static_assets(&root)?;
    seed_theme_metadata(&root)?;
    seed_sample_post(&root)?;

    println!("Initialized");
    Ok(())
}

fn establish_directories(root: &Path) -> Result<()> {
    for entry in DIRECTORIES {
        let path = root.join(entry);
        if path.exists() {
            continue;
        }
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create directory {}", path.display()))?;
    }
    Ok(())
}

fn seed_configuration(root: &Path) -> Result<()> {
    let destination = root.join(CONFIG_FILE);
    write_if_missing(&destination, DEFAULT_CONFIG)
        .with_context(|| format!("failed to write {}", CONFIG_FILE))
}

fn seed_theme(root: &Path) -> Result<()> {
    let theme_root = root.join("themes").join(THEME_NAME);
    fs::create_dir_all(theme_root.join("templates")).with_context(|| {
        format!(
            "failed to create theme templates directory at {}",
            theme_root.join("templates").display()
        )
    })?;
    fs::create_dir_all(theme_root.join("skel")).with_context(|| {
        format!(
            "failed to create theme skel directory at {}",
            theme_root.join("skel").display()
        )
    })?;
    Ok(())
}

fn seed_theme_metadata(root: &Path) -> Result<()> {
    let theme_root = root.join("themes").join(THEME_NAME);
    write_if_missing(&theme_root.join("theme.yaml"), THEME_MANIFEST)
        .context("failed to write theme manifest")?;
    write_if_missing(&theme_root.join("style.tailwind.css"), THEME_STYLE_SOURCE)
        .context("failed to write theme CSS source")?;
    write_if_missing(
        &theme_root.join("tailwind.config.js"),
        THEME_TAILWIND_CONFIG,
    )
    .context("failed to write theme Tailwind config")?;

    write_if_missing(&root.join("style.tailwind.css"), THEME_STYLE_SOURCE)
        .context("failed to write style.tailwind.css")?;
    write_if_missing(&root.join("tailwind.config.js"), THEME_TAILWIND_CONFIG)
        .context("failed to write tailwind.config.js")?;

    Ok(())
}

fn seed_templates(root: &Path) -> Result<()> {
    let template_pairs = [
        ("base.html", THEME_BASE_TEMPLATE),
        ("post.html", THEME_POST_TEMPLATE),
        ("index.html", THEME_INDEX_TEMPLATE),
        ("tag.html", THEME_TAG_TEMPLATE),
        ("archive_year.html", THEME_ARCHIVE_YEAR_TEMPLATE),
        ("archive_month.html", THEME_ARCHIVE_MONTH_TEMPLATE),
        ("rss.xml", THEME_RSS_TEMPLATE),
    ];

    for (name, contents) in template_pairs {
        let theme_dest = root
            .join("themes")
            .join(THEME_NAME)
            .join("templates")
            .join(name);
        write_if_missing(&theme_dest, contents)
            .with_context(|| format!("failed to write theme template {name}"))?;

        let project_dest = root.join("templates").join(name);
        write_if_missing(&project_dest, contents)
            .with_context(|| format!("failed to write templates/{name}"))?;
    }

    Ok(())
}

fn seed_sample_post(root: &Path) -> Result<()> {
    let sample_dir = root.join(
        ["posts", "hello-from-bucket3rs"]
            .into_iter()
            .collect::<PathBuf>(),
    );
    if !sample_dir.exists() {
        fs::create_dir_all(&sample_dir)
            .with_context(|| format!("failed to create {}", sample_dir.display()))?;
    }
    write_if_missing(&sample_dir.join("post.md"), SAMPLE_POST)
        .context("failed to write sample post")
}

fn seed_static_assets(root: &Path) -> Result<()> {
    let asset_pairs = [
        (PathBuf::from("style.css"), THEME_STYLE_CSS),
        (PathBuf::from("style.tailwind.css"), THEME_STYLE_SOURCE),
    ];

    for (relative, contents) in asset_pairs {
        let theme_dest = root
            .join("themes")
            .join(THEME_NAME)
            .join("skel")
            .join(&relative);
        write_if_missing(&theme_dest, contents)
            .with_context(|| format!("failed to write theme asset {}", relative.display()))?;

        let project_dest = root.join("skel").join(&relative);
        write_if_missing(&project_dest, contents)
            .with_context(|| format!("failed to write skel/{}", relative.display()))?;
    }

    Ok(())
}

fn write_if_missing(path: &Path, contents: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut file =
        fs::File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}
