use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const DIRECTORIES: &[&str] = &["html", "posts", "templates", "skel", "themes"];
const CONFIG_FILE: &str = "bucket3.yaml";
const THEME_NAME: &str = "bckt3";

mod embedded_theme {
    include!(concat!(env!("OUT_DIR"), "/theme_bckt3.rs"));
}

use embedded_theme::{EmbeddedFile, THEME_BCKT3_FILES};

const DEFAULT_CONFIG: &str = r#"title: "My Bucket3 Site"
base_url: "https://example.com"
homepage_posts: 5
date_format: "[year]-[month]-[day]"
paginate_tags: true
default_timezone: "+00:00"
theme: bckt3
"#;

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
    copy_theme_contents(&theme_root)
}

fn seed_templates(root: &Path) -> Result<()> {
    copy_theme_subset("templates/", &root.join("templates"))?;
    copy_theme_subset("pages/", &root.join("pages"))
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
    copy_theme_subset("skel/", &root.join("skel"))
}

fn write_if_missing(path: &Path, contents: &str) -> Result<()> {
    write_bytes_if_missing(path, contents.as_bytes())
}

fn copy_theme_contents(theme_root: &Path) -> Result<()> {
    if !theme_root.exists() {
        fs::create_dir_all(theme_root)
            .with_context(|| format!("failed to create {}", theme_root.display()))?;
    }

    for file in THEME_BCKT3_FILES {
        let destination = theme_root.join(file.path);
        copy_embedded_file(&destination, file)?;
    }

    Ok(())
}

fn copy_theme_subset(prefix: &str, destination_root: &Path) -> Result<()> {
    let normalized = normalize_prefix(prefix);
    for file in THEME_BCKT3_FILES
        .iter()
        .filter(|file| file.path.starts_with(&normalized))
    {
        let relative = &file.path[normalized.len()..];
        if relative.is_empty() {
            continue;
        }
        let destination = destination_root.join(relative);
        copy_embedded_file(&destination, file)?;
    }

    Ok(())
}

fn copy_embedded_file(destination: &Path, file: &EmbeddedFile) -> Result<()> {
    write_bytes_if_missing(destination, file.contents)
}

fn write_bytes_if_missing(path: &Path, contents: &[u8]) -> Result<()> {
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
    file.write_all(contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn normalize_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim_start_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.ends_with('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}/")
    }
}
