use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use minijinja::Environment;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description;
use time::format_description::well_known::Rfc3339;
use walkdir::WalkDir;

use crate::config::Config;
use crate::content::{Post, discover_posts};
use crate::template;

pub struct RenderPlan {
    pub posts: bool,
    pub static_assets: bool,
}

pub fn render_site(root: &Path, plan: RenderPlan) -> Result<()> {
    let config_path = root.join("bucket3.yaml");
    let config = Config::load(&config_path)?;
    let html_root = root.join("html");
    fs::create_dir_all(&html_root).context("failed to ensure html directory exists")?;

    if plan.posts {
        let mut env = template::environment(&config)?;
        load_templates(root, &mut env)?;
        render_posts(root, &html_root, &config, &env)?;
    }

    if plan.static_assets {
        copy_static_assets(root, &html_root)?;
    }

    Ok(())
}

fn load_templates(root: &Path, env: &mut Environment<'static>) -> Result<()> {
    let templates_dir = root.join("templates");
    if !templates_dir.exists() {
        bail!("templates directory {} not found", templates_dir.display());
    }

    for entry in WalkDir::new(&templates_dir) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let template_body = fs::read_to_string(entry.path())
            .with_context(|| format!("failed to read template {}", entry.path().display()))?;
        let relative_name = entry
            .path()
            .strip_prefix(&templates_dir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let name_static = Box::leak(relative_name.clone().into_boxed_str());
        let template_static = Box::leak(template_body.into_boxed_str());
        env.add_template(name_static, template_static)
            .with_context(|| format!("failed to register template {}", relative_name))?;
    }

    Ok(())
}

fn render_posts(
    root: &Path,
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
) -> Result<()> {
    let posts_dir = root.join("posts");
    let posts = discover_posts(&posts_dir)?;
    if posts.is_empty() {
        return Ok(());
    }

    let post_template = env
        .get_template("post.html")
        .context("post.html template missing")?;

    for post in posts {
        let render_target = html_root.join(post.permalink.trim_start_matches('/'));
        let output_path = render_target.join("index.html");
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let context = build_post_context(config, &post)?;
        let rendered = post_template
            .render(minijinja::context! { post => context })
            .with_context(|| format!("failed to render template for {}", post.slug))?;

        fs::write(&output_path, rendered)
            .with_context(|| format!("failed to write {}", output_path.display()))?;

        copy_post_assets(&post, &render_target)
            .with_context(|| format!("failed to copy assets for {}", post.slug))?;
    }

    Ok(())
}

fn build_post_context(config: &Config, post: &Post) -> Result<PostTemplate> {
    let display_date = format_date(config, &post.date)?;
    let iso_date = post
        .date
        .format(&Rfc3339)
        .context("failed to format RFC3339 date")?;

    let attached = convert_paths(&post.attached)?;
    let images = convert_paths(&post.images)?;

    Ok(PostTemplate {
        title: post.title.clone(),
        slug: post.slug.clone(),
        date: display_date,
        date_iso: iso_date,
        tags: post.tags.clone(),
        abstract_text: post.abstract_text.clone(),
        attached,
        body: post.body_html.clone(),
        excerpt: post.excerpt.clone(),
        images,
        video_url: post.video_url.clone(),
        permalink: post.permalink.clone(),
    })
}

fn format_date(config: &Config, date: &OffsetDateTime) -> Result<String> {
    if config.date_format.eq_ignore_ascii_case("RFC3339") {
        return date
            .format(&Rfc3339)
            .context("failed to format RFC3339 date");
    }

    let description = format_description::parse(&config.date_format)
        .with_context(|| format!("invalid date_format '{}'", config.date_format))?;
    date.format(&description).with_context(|| {
        format!(
            "failed to format date with pattern '{}'",
            config.date_format
        )
    })
}

fn convert_paths(paths: &[PathBuf]) -> Result<Vec<String>> {
    let mut set = BTreeSet::new();
    for path in paths {
        if path.is_absolute() {
            bail!("asset paths must be relative: {}", path.display());
        }
        set.insert(normalize_path(path));
    }
    Ok(set.into_iter().collect())
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|comp| comp.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn copy_post_assets(post: &Post, target_dir: &Path) -> Result<()> {
    let mut assets = BTreeSet::new();
    for entry in post.attached.iter().chain(post.images.iter()) {
        if entry.is_absolute() {
            bail!("{}: asset path must be relative", entry.display());
        }
        assets.insert(entry.clone());
    }

    for relative in assets {
        let source = post.source_dir.join(&relative);
        if !source.exists() {
            bail!("missing asset {}", source.display());
        }
        let destination = target_dir.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(&source, &destination).with_context(|| {
            format!(
                "failed to copy asset from {} to {}",
                source.display(),
                destination.display()
            )
        })?;
    }

    Ok(())
}

fn copy_static_assets(root: &Path, html_root: &Path) -> Result<()> {
    let skel_dir = root.join("skel");
    if !skel_dir.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(&skel_dir) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        let relative = entry.path().strip_prefix(&skel_dir).unwrap();
        let destination = html_root.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(entry.path(), &destination).with_context(|| {
            format!(
                "failed to copy static asset from {} to {}",
                entry.path().display(),
                destination.display()
            )
        })?;
    }

    Ok(())
}

#[derive(Serialize)]
struct PostTemplate {
    title: Option<String>,
    slug: String,
    date: String,
    date_iso: String,
    tags: Vec<String>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    attached: Vec<String>,
    body: String,
    excerpt: String,
    images: Vec<String>,
    video_url: Option<String>,
    permalink: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_template(root: &Path, name: &str, contents: &str) {
        let path = root.join("templates").join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn setup_markdown_templates(root: &Path) {
        write_template(
            root,
            "base.html",
            "<!doctype html><html><body>{% block content %}{% endblock %}</body></html>",
        );
        write_template(
            root,
            "post.html",
            "{% extends \"base.html\" %}{% block content %}<article>{{ post.title }}|{{ post.body | safe }}|{{ post.date }}|{{ post.excerpt }}</article>{% endblock %}",
        );
    }

    fn write_markdown_post(root: &Path, body: &str) {
        let post_dir = root.join("posts/hello-world");
        fs::create_dir_all(&post_dir).unwrap();
        fs::write(
            post_dir.join("post.md"),
            format!(
                "---\ntitle: Example\ndate: 2024-01-02T03:04:05Z\ntags: [test]\n---\n{}",
                body
            ),
        )
        .unwrap();
    }

    #[test]
    fn renders_markdown_post_to_expected_location() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        fs::create_dir_all(root.join("skel")).unwrap();
        setup_markdown_templates(root);
        write_markdown_post(root, "Hello **world**!");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
            },
        )
        .unwrap();

        let output = root.join("html/2024/01/02/hello-world/index.html");
        let rendered = fs::read_to_string(output).unwrap();
        assert!(rendered.contains("Example"));
        assert!(rendered.contains("<strong>world</strong>"));
        assert!(rendered.contains("Hello world"));
    }

    #[test]
    fn copies_post_assets() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts/assets-post")).unwrap();
        setup_markdown_templates(root);
        fs::write(root.join("posts/assets-post/post.md"), "---\ndate: 2024-01-01T00:00:00Z\nattached: [data/notes.txt]\nimages: [images/pic.png]\n---\nBody").unwrap();
        fs::create_dir_all(root.join("posts/assets-post/data")).unwrap();
        fs::create_dir_all(root.join("posts/assets-post/images")).unwrap();
        fs::write(root.join("posts/assets-post/data/notes.txt"), "notes").unwrap();
        fs::write(root.join("posts/assets-post/images/pic.png"), "image").unwrap();

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
            },
        )
        .unwrap();

        let asset = root.join("html/2024/01/01/assets-post/data/notes.txt");
        let image = root.join("html/2024/01/01/assets-post/images/pic.png");
        assert!(asset.exists());
        assert!(image.exists());
    }

    #[test]
    fn copies_static_assets() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("skel/css")).unwrap();
        fs::write(root.join("skel/css/site.css"), "body { color: black; }").unwrap();

        render_site(
            root,
            RenderPlan {
                posts: false,
                static_assets: true,
            },
        )
        .unwrap();

        let copied = root.join("html/css/site.css");
        assert!(copied.exists());
    }
}
