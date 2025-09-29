use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const DIRECTORIES: &[&str] = &["html", "posts", "templates", "skel"];
const CONFIG_FILE: &str = "bucket3.yaml";

const DEFAULT_CONFIG: &str = r#"title: "My Bucket3 Site"
base_url: "https://example.com"
homepage_posts: 5
date_format: "[year]-[month]-[day]"
"#;

const BASE_TEMPLATE: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>{{ title | default(config.title | default("bucket3")) }}</title>
</head>
<body>
  <main>
    {% block content %}{% endblock content %}
  </main>
</body>
</html>
"#;

const POST_TEMPLATE: &str = r#"{% extends "base.html" %}
{% block content %}
<article>
  {% if post.title %}<h1>{{ post.title }}</h1>{% endif %}
  <div>{{ post.body | safe }}</div>
</article>
{% endblock content %}
"#;

const INDEX_TEMPLATE: &str = r#"{% extends "base.html" %}
{% block content %}
<section>
  <h1>Recent Posts</h1>
  {% for post in posts %}
  <article>
    {% if post.title %}<h2>{{ post.title }}</h2>{% endif %}
    <div>{{ post.body | safe }}</div>
  </article>
  {% else %}
  <p>No posts yet.</p>
  {% endfor %}
</section>
{% endblock content %}
"#;

const SAMPLE_POST: &str = r#"---
title: "Hello From bucket3rs"
slug: "hello-from-bucket3rs"
date: "2024-01-01T00:00:00Z"
tags:
  - welcome
abstract: "Kick the tires on the generator."
attached: []
---

This is the starter post. Edit it or drop in your own content to get going.
"#;

const SAMPLE_STYLE: &str = r#"body {
  margin: 0;
  font-family: system-ui, sans-serif;
  line-height: 1.5;
}

main {
  max-width: 720px;
  margin: 0 auto;
  padding: 1.5rem;
}
"#;

pub fn run_init_command() -> Result<()> {
    let root = env::current_dir().context("failed to resolve current directory")?;

    establish_directories(&root)?;
    seed_configuration(&root)?;
    seed_templates(&root)?;
    seed_sample_post(&root)?;
    seed_static_assets(&root)?;

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

fn seed_templates(root: &Path) -> Result<()> {
    let templates = root.join("templates");
    write_if_missing(&templates.join("base.html"), BASE_TEMPLATE)
        .context("failed to write templates/base.html")?;
    write_if_missing(&templates.join("post.html"), POST_TEMPLATE)
        .context("failed to write templates/post.html")?;
    write_if_missing(&templates.join("index.html"), INDEX_TEMPLATE)
        .context("failed to write templates/index.html")?;
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
    let style_path = root.join(["skel", "style.css"].into_iter().collect::<PathBuf>());
    write_if_missing(&style_path, SAMPLE_STYLE).context("failed to write skel/style.css")
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
