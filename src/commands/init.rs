use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use walkdir::WalkDir;

use crate::cli::InitArgs;
use crate::theme::{GithubReference, ThemeSource, download_theme};

const DIRECTORIES: &[&str] = &["html", "posts", "templates", "skel", "themes", "pages"];
const CONFIG_FILE: &str = "bckt.yaml";
const DEFAULT_THEME_NAME: &str = "bckt3";
const DEFAULT_THEME_SUBDIR: &str = "themes/bckt3";
const DEFAULT_THEME_OWNER: &str = "vrypan";
const DEFAULT_THEME_REPO: &str = "bckt";

const DEFAULT_CONFIG_TEMPLATE: &str = r#"title: "My bckt Site"
base_url: "https://example.com"
homepage_posts: 5
date_format: "[year]-[month]-[day]"
paginate_tags: true
default_timezone: "+00:00"
theme: {theme}
"#;

const SAMPLE_POST: &str = r#"---
title: "Hello From bckt"
slug: "hello-from-bckt"
date: "2024-01-01T00:00:00Z"
tags:
  - welcome
abstract: "Kick the tires on the generator."
attached: []
images: []
---

This is the starter post. Edit it or drop in your own content to get going.
"#;

pub fn run_init_command(args: InitArgs) -> Result<()> {
    let root = env::current_dir().context("failed to resolve current directory")?;

    establish_directories(&root)?;

    let theme_name = args
        .theme_name
        .clone()
        .unwrap_or_else(|| DEFAULT_THEME_NAME.to_string());
    let theme_dir = root.join("themes").join(&theme_name);

    ensure_theme(&theme_dir, &args)?;

    seed_configuration(&root, &theme_name)?;
    seed_templates(&root, &theme_dir)?;
    seed_static_assets(&root, &theme_dir)?;
    seed_sample_post(&root)?;

    println!("Initialized project with theme '{theme_name}'");
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

fn ensure_theme(theme_dir: &Path, args: &InitArgs) -> Result<()> {
    if theme_dir.exists() {
        return Ok(());
    }

    let source = if let Some(url) = &args.theme_url {
        ThemeSource::Url {
            url: url.clone(),
            subdir: args.theme_subdir.clone(),
            strip_components: args.strip_components,
        }
    } else if let Some(repo_spec) = &args.theme_github {
        let (owner, repo) = split_owner_repo(repo_spec)?;
        let reference = select_reference(args.theme_tag.clone(), args.theme_branch.clone());
        let subdir = args.theme_subdir.clone();
        ThemeSource::Github {
            owner,
            repo,
            reference,
            subdir,
            strip_components: Some(args.strip_components.unwrap_or(1)),
        }
    } else {
        let subdir = args
            .theme_subdir
            .clone()
            .unwrap_or_else(|| DEFAULT_THEME_SUBDIR.to_string());
        let strip = args.strip_components.unwrap_or(1);
        let default_tag = format!("v{}", env!("CARGO_PKG_VERSION"));
        let default_source = ThemeSource::Github {
            owner: DEFAULT_THEME_OWNER.to_string(),
            repo: DEFAULT_THEME_REPO.to_string(),
            reference: GithubReference::Tag(default_tag.clone()),
            subdir: Some(subdir.clone()),
            strip_components: Some(strip),
        };

        if let Err(err) = download_theme(theme_dir, default_source.clone()) {
            eprintln!(
                "Warning: failed to download default theme tag {default_tag}: {err}. Falling back to main branch."
            );
            ThemeSource::Github {
                owner: DEFAULT_THEME_OWNER.to_string(),
                repo: DEFAULT_THEME_REPO.to_string(),
                reference: GithubReference::Branch("main".to_string()),
                subdir: Some(subdir),
                strip_components: Some(strip),
            }
        } else {
            return Ok(());
        }
    };

    download_theme(theme_dir, source)
}

fn select_reference(tag: Option<String>, branch: Option<String>) -> GithubReference {
    match (tag, branch) {
        (Some(tag), _) => GithubReference::Tag(tag),
        (None, Some(branch)) => GithubReference::Branch(branch),
        (None, None) => GithubReference::Branch("main".to_string()),
    }
}

fn split_owner_repo(spec: &str) -> Result<(String, String)> {
    let mut parts = spec.splitn(2, '/');
    let owner = parts
        .next()
        .ok_or_else(|| anyhow!("missing owner in GitHub specification"))?;
    let repo = parts
        .next()
        .ok_or_else(|| anyhow!("missing repository name in GitHub specification"))?;
    if owner.is_empty() || repo.is_empty() {
        return Err(anyhow!("invalid GitHub specification '{spec}'"));
    }
    Ok((owner.to_string(), repo.to_string()))
}

fn seed_configuration(root: &Path, theme_name: &str) -> Result<()> {
    let destination = root.join(CONFIG_FILE);
    if destination.exists() {
        return Ok(());
    }
    let contents = DEFAULT_CONFIG_TEMPLATE.replace("{theme}", theme_name);
    write_if_missing(&destination, &contents)
        .with_context(|| format!("failed to write {}", CONFIG_FILE))
}

fn seed_templates(root: &Path, theme_root: &Path) -> Result<()> {
    let source = theme_root.join("templates");
    copy_if_missing(&source, &root.join("templates"))?;

    let pages = theme_root.join("pages");
    copy_if_missing(&pages, &root.join("pages"))
}

fn seed_static_assets(root: &Path, theme_root: &Path) -> Result<()> {
    let source = theme_root.join("skel");
    copy_if_missing(&source, &root.join("skel"))
}

fn seed_sample_post(root: &Path) -> Result<()> {
    let sample_dir = root.join(
        ["posts", "hello-from-bckt"]
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

fn write_if_missing(path: &Path, contents: &str) -> Result<()> {
    write_bytes_if_missing(path, contents.as_bytes())
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
    file.flush()
        .with_context(|| format!("failed to flush {}", path.display()))?;
    Ok(())
}

fn copy_if_missing(source_root: &Path, destination_root: &Path) -> Result<()> {
    if !source_root.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(source_root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if entry.file_type().is_dir() {
            continue;
        }
        let relative = path
            .strip_prefix(source_root)
            .with_context(|| format!("failed to strip prefix for {}", path.display()))?;
        let destination = destination_root.join(relative);
        if destination.exists() {
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(path, &destination).with_context(|| {
            format!(
                "failed to copy {} to {}",
                path.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
}
