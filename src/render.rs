use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use minijinja::Environment;
use serde::{Deserialize, Serialize};
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

const CACHE_DIR: &str = ".bucket3/cache";
const HOME_PAGES_KEY: &str = "home_pages";

#[derive(Clone, Serialize, Deserialize)]
struct StoredPage {
    cursor: String,
    posts: Vec<String>,
}

struct HomePageCache {
    db: sled::Db,
}

impl HomePageCache {
    fn open(root: &Path) -> Result<Self> {
        let cache_dir = root.join(CACHE_DIR);
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("failed to create cache directory {}", cache_dir.display()))?;
        let db = sled::open(cache_dir.join("sled")).context("failed to open cache database")?;
        Ok(Self { db })
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

pub fn render_site(root: &Path, plan: RenderPlan) -> Result<()> {
    let config_path = root.join("bucket3.yaml");
    let config = Config::load(&config_path)?;
    let html_root = root.join("html");
    fs::create_dir_all(&html_root).context("failed to ensure html directory exists")?;

    let cache = HomePageCache::open(root)?;
    let mut env = template::environment(&config)?;
    load_templates(root, &mut env)?;

    let posts = if plan.posts {
        render_posts(root, &html_root, &config, &env)?
    } else {
        Vec::new()
    };

    if plan.posts {
        render_homepage(&posts, &html_root, &config, &env, &cache)?;
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
) -> Result<Vec<Post>> {
    let posts_dir = root.join("posts");
    let mut posts = discover_posts(&posts_dir)?;
    if posts.is_empty() {
        return Ok(posts);
    }

    posts.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.slug.cmp(&b.slug)));

    let post_template = env
        .get_template("post.html")
        .context("post.html template missing")?;

    for post in &posts {
        let render_target = html_root.join(post.permalink.trim_start_matches('/'));
        let output_path = render_target.join("index.html");
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let context = build_post_context(config, post)?;
        let rendered = post_template
            .render(minijinja::context! { post => context })
            .with_context(|| format!("failed to render template for {}", post.slug))?;

        fs::write(&output_path, rendered)
            .with_context(|| format!("failed to write {}", output_path.display()))?;

        copy_post_assets(post, &render_target)
            .with_context(|| format!("failed to copy assets for {}", post.slug))?;
    }

    Ok(posts)
}

fn render_homepage(
    posts: &[Post],
    html_root: &Path,
    config: &Config,
    env: &Environment<'static>,
    cache: &HomePageCache,
) -> Result<()> {
    if posts.is_empty() {
        cache.store_pages(&[])?;
        return Ok(());
    }

    let template = env
        .get_template("index.html")
        .context("index.html template missing")?;

    let per_page = std::cmp::max(1, config.homepage_posts);
    let mut lookup: HashMap<String, &Post> = HashMap::new();
    for post in posts {
        lookup.insert(post_key(post), post);
    }

    let head_slice: Vec<&Post> = posts.iter().take(per_page).collect();
    if head_slice.is_empty() {
        return Ok(());
    }
    let head_cursor = page_cursor(head_slice.last().unwrap());
    let head_posts: Vec<String> = head_slice.iter().map(|p| post_key(p)).collect();

    let mut stored_pages = cache.load_pages()?;
    let previous_head_cursor = stored_pages.first().map(|page| page.cursor.clone());

    stored_pages.retain(|page| page.cursor != head_cursor);

    let mut new_records = Vec::new();
    new_records.push(StoredPage {
        cursor: head_cursor.clone(),
        posts: head_posts.clone(),
    });

    let mut known_ids: HashSet<String> = stored_pages
        .iter()
        .flat_map(|page| page.posts.iter().cloned())
        .collect();
    for id in &head_posts {
        known_ids.insert(id.clone());
    }

    let mut buffer: Vec<&Post> = Vec::new();
    let mut extra_chunks: Vec<Vec<&Post>> = Vec::new();
    for post in posts.iter().skip(per_page) {
        let id = post_key(post);
        if known_ids.contains(&id) {
            continue;
        }
        buffer.push(post);
        known_ids.insert(id);
        if buffer.len() == per_page {
            extra_chunks.push(buffer.clone());
            buffer.clear();
        }
    }
    if !buffer.is_empty() {
        extra_chunks.push(buffer);
    }

    let mut seen_cursors: HashSet<String> = HashSet::new();
    seen_cursors.insert(head_cursor.clone());

    for chunk in &extra_chunks {
        if let Some(last) = chunk.last() {
            let cursor = page_cursor(last);
            if seen_cursors.insert(cursor.clone()) {
                let ids = chunk.iter().map(|p| post_key(p)).collect::<Vec<_>>();
                new_records.push(StoredPage { cursor, posts: ids });
            }
        }
    }

    for page in stored_pages.iter() {
        if seen_cursors.insert(page.cursor.clone()) {
            new_records.push(page.clone());
        }
    }

    let head_changed = previous_head_cursor
        .map(|cursor| cursor != head_cursor)
        .unwrap_or(true);

    let mut plans: Vec<PagePlan> = Vec::new();

    for (index, record) in new_records.iter().enumerate() {
        let summaries = record
            .posts
            .iter()
            .filter_map(|id| lookup.get(id))
            .map(|post| build_post_summary(config, post))
            .collect::<Result<Vec<_>>>()?;

        let prev = if index == 0 {
            String::new()
        } else if index == 1 {
            "/".to_string()
        } else {
            page_url(&new_records[index - 1].cursor)
        };
        let next = if index + 1 < new_records.len() {
            page_url(&new_records[index + 1].cursor)
        } else {
            String::new()
        };

        let pagination = PaginationContext {
            current: index + 1,
            total: new_records.len(),
            prev,
            next,
        };

        let outputs = if index == 0 {
            vec![html_root.join("index.html")]
        } else {
            vec![page_output_path(html_root, &record.cursor)]
        };

        let exists_previously = stored_pages.iter().any(|page| page.cursor == record.cursor);
        let mut needs_render = index == 0 || !exists_previously;
        if !needs_render {
            if head_changed && index == 1 {
                needs_render = true;
            } else {
                let path = page_output_path(html_root, &record.cursor);
                needs_render = !path.exists();
            }
        }

        if needs_render {
            plans.push(PagePlan {
                summaries,
                pagination,
                outputs,
            });
        }
    }

    for plan in plans {
        render_page(&template, plan)?;
    }

    cache.store_pages(&new_records)?;

    Ok(())
}

fn render_page(template: &minijinja::Template<'_, '_>, plan: PagePlan) -> Result<()> {
    let rendered = template
        .render(minijinja::context! { posts => plan.summaries, pagination => plan.pagination })
        .context("failed to render homepage")?;

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

fn build_post_summary(config: &Config, post: &Post) -> Result<PostSummary> {
    let date = format_date(config, &post.date)?;
    let date_iso = post
        .date
        .format(&Rfc3339)
        .context("failed to format RFC3339 date")?;

    Ok(PostSummary {
        title: post.title.clone(),
        slug: post.slug.clone(),
        date,
        date_iso,
        body: post.body_html.clone(),
        excerpt: post.excerpt.clone(),
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

fn post_key(post: &Post) -> String {
    format!("{}-{}", post.date.unix_timestamp(), post.slug)
}

fn page_cursor(post: &Post) -> String {
    post_key(post)
}

fn page_url(cursor: &str) -> String {
    format!("/page/{}/", cursor)
}

fn page_output_path(html_root: &Path, cursor: &str) -> PathBuf {
    html_root.join("page").join(cursor).join("index.html")
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

#[derive(Serialize)]
struct PostSummary {
    title: Option<String>,
    slug: String,
    date: String,
    date_iso: String,
    body: String,
    excerpt: String,
    permalink: String,
}

#[derive(Serialize)]
struct PaginationContext {
    current: usize,
    total: usize,
    prev: String,
    next: String,
}

struct PagePlan {
    summaries: Vec<PostSummary>,
    pagination: PaginationContext,
    outputs: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

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
        write_template(
            root,
            "index.html",
            "{% extends \"base.html\" %}{% block content %}<section data-current=\"{{ pagination.current }}\" data-total=\"{{ pagination.total }}\" data-prev=\"{{ pagination.prev | safe }}\" data-next=\"{{ pagination.next | safe }}\">{% for post in posts %}<article data-slug=\"{{ post.slug }}\"></article>{% endfor %}</section>{% endblock %}",
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

    fn write_dated_post(root: &Path, slug: &str, date: &str, body: &str) {
        let dir = root.join("posts").join(slug);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("post.md"),
            format!(
                "---\ntitle: {0}\ndate: {1}\nslug: {0}\n---\n{2}",
                slug, date, body
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

        let homepage = fs::read_to_string(root.join("html/index.html")).unwrap();
        assert!(homepage.contains("article data-slug=\"hello-world\""));
        assert!(homepage.contains("data-current=\"1\""));
        assert!(homepage.contains("data-total=\"1\""));
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
        setup_markdown_templates(root);

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

    #[test]
    fn paginates_homepage_cursor_based() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("posts")).unwrap();
        setup_markdown_templates(root);
        fs::write(root.join("bucket3.yaml"), "homepage_posts: 1\n").unwrap();

        write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "A");
        write_dated_post(root, "beta", "2024-02-01T00:00:00Z", "B");
        write_dated_post(root, "gamma", "2024-03-01T00:00:00Z", "C");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
            },
        )
        .unwrap();

        let ts_gamma = OffsetDateTime::parse("2024-03-01T00:00:00Z", &Rfc3339)
            .unwrap()
            .unix_timestamp();
        let ts_beta = OffsetDateTime::parse("2024-02-01T00:00:00Z", &Rfc3339)
            .unwrap()
            .unix_timestamp();
        let ts_alpha = OffsetDateTime::parse("2024-01-01T00:00:00Z", &Rfc3339)
            .unwrap()
            .unix_timestamp();

        let index = fs::read_to_string(root.join("html/index.html")).unwrap();
        assert!(index.contains("article data-slug=\"gamma\""));
        assert!(index.contains(&format!("data-next=\"/page/{ts_beta}-beta/\"")));

        let second =
            fs::read_to_string(root.join(format!("html/page/{ts_beta}-beta/index.html"))).unwrap();
        assert!(second.contains("article data-slug=\"beta\""));
        assert!(second.contains("data-prev=\"/\""));
        assert!(second.contains(&format!("data-next=\"/page/{ts_alpha}-alpha/\"")));

        let third = fs::read_to_string(root.join(format!("html/page/{ts_alpha}-alpha/index.html")))
            .unwrap();
        assert!(third.contains("article data-slug=\"alpha\""));
        assert!(third.contains("data-prev=\"/page/"));
        assert!(third.contains("data-next=\"\""));

        // Add a new post and ensure only new pages are added with stable cursors
        write_dated_post(root, "delta", "2024-04-01T00:00:00Z", "D");

        render_site(
            root,
            RenderPlan {
                posts: true,
                static_assets: false,
            },
        )
        .unwrap();

        let refreshed_index = fs::read_to_string(root.join("html/index.html")).unwrap();
        assert!(refreshed_index.contains("article data-slug=\"delta\""));
        assert!(refreshed_index.contains(&format!("data-next=\"/page/{ts_gamma}-gamma/\"")));

        let archived = root.join(format!("html/page/{ts_beta}-beta/index.html"));
        assert!(archived.exists());
    }
}
