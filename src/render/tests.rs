use super::*;
use std::fs;
use std::time::UNIX_EPOCH;
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
    write_template(
        root,
        "index.html",
        "{% extends \"base.html\" %}{% block content %}<section data-current=\"{{ pagination.current }}\" data-total=\"{{ pagination.total }}\" data-prev=\"{{ pagination.prev | safe }}\" data-next=\"{{ pagination.next | safe }}\">{% for post in posts %}<article data-slug=\"{{ post.slug }}\"></article>{% endfor %}</section>{% endblock %}",
    );
    write_template(
        root,
        "tag.html",
        "{% extends \"base.html\" %}{% block content %}<section data-tag=\"{{ tag }}\" data-current=\"{{ pagination.current }}\" data-total=\"{{ pagination.total }}\" data-prev=\"{{ pagination.prev | safe }}\" data-next=\"{{ pagination.next | safe }}\">{% for post in posts %}<article data-slug=\"{{ post.slug }}\"></article>{% endfor %}</section>{% endblock %}",
    );
    write_template(
        root,
        "archive_year.html",
        "{% extends \"base.html\" %}{% block content %}<section data-year=\"{{ year }}\">{% for post in posts %}<article data-slug=\"{{ post.slug }}\"></article>{% endfor %}</section>{% endblock %}",
    );
    write_template(
        root,
        "archive_month.html",
        "{% extends \"base.html\" %}{% block content %}<section data-year=\"{{ year }}\" data-month=\"{{ month }}\">{% for post in posts %}<article data-slug=\"{{ post.slug }}\"></article>{% endfor %}</section>{% endblock %}",
    );
    write_template(
        root,
        "rss.xml",
        "{% autoescape false %}\n<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<rss version=\"2.0\" xmlns:content=\"http://purl.org/rss/1.0/modules/content/\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n  <channel>\n    <title>{{ feed.title }}</title>\n    <link>{{ feed.site_url }}</link>\n    <description>{{ feed.description }}</description>\n    <lastBuildDate>{{ feed.updated }}</lastBuildDate>\n    <generator>bckt</generator>\n    <atom:link href=\"{{ feed.feed_url }}\" rel=\"self\" type=\"application/rss+xml\"/>\n    {% for item in feed.items %}\n    <item>\n      <title>{{ item.title | default(value=item.slug) }}</title>\n      <link>{{ base_url }}{{ item.permalink }}</link>\n      <guid isPermaLink=\"true\">{{ base_url }}{{ item.permalink }}</guid>\n      <pubDate>{{ item.pub_date }}</pubDate>\n      <description>{{ item.excerpt | default(value=item.title | default(value=item.slug)) }}</description>\n      <content:encoded><![CDATA[{{ item.body }}]]></content:encoded>\n    </item>\n    {% endfor %}\n  </channel>\n</rss>\n{% endautoescape %}\n",
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

fn write_tagged_post(root: &Path, slug: &str, tag: &str, date: &str, body: &str) {
    let dir = root.join("posts").join(slug);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("post.md"),
        format!(
            "---\ntitle: {0}\ndate: {2}\nslug: {0}\ntags:\n  - {1}\n---\n{3}",
            slug, tag, date, body
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
            "---\ntitle: {0}\ndate: {1}\nslug: {0}\ntags:\n  - {0}\n---\n{2}",
            slug, date, body
        ),
    )
    .unwrap();
}

fn file_mtime(path: &Path) -> std::time::Duration {
    fs::metadata(path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
}

fn wait_for_filesystem_tick() {
    std::thread::sleep(std::time::Duration::from_millis(1100));
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
            mode: BuildMode::Full,
            verbose: false,
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
    fs::write(
        root.join("posts/assets-post/post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\nattached: [data/notes.txt, images/pic.png]\n---\nBody",
    )
    .unwrap();
    fs::create_dir_all(root.join("posts/assets-post/data")).unwrap();
    fs::create_dir_all(root.join("posts/assets-post/images")).unwrap();
    fs::write(root.join("posts/assets-post/data/notes.txt"), "notes").unwrap();
    fs::write(root.join("posts/assets-post/images/pic.png"), "image").unwrap();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let asset = root.join("html/2024/01/01/assets-post/data/notes.txt");
    let image = root.join("html/2024/01/01/assets-post/images/pic.png");
    assert!(asset.exists());
    assert!(image.exists());
}

#[test]
fn renders_pages_from_pages_directory() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    setup_markdown_templates(root);
    fs::create_dir_all(root.join("pages/about")).unwrap();
    fs::create_dir_all(root.join("pages/features/page1/assets")).unwrap();
    fs::write(
        root.join("pages/404.html"),
        "{% extends \"base.html\" %}{% block content %}<h1>Missing</h1>{% endblock %}",
    )
    .unwrap();
    fs::write(
            root.join("pages/about/index.html"),
            "{% extends \"base.html\" %}{% block content %}<p>About {{ config.title | default('site') }}</p>{% endblock %}",
        )
        .unwrap();
    fs::write(
        root.join("pages/features/page1/index.html"),
        "{% extends \"base.html\" %}{% block content %}<p>Feature</p>{% endblock %}",
    )
    .unwrap();
    fs::write(root.join("pages/features/page1/diagram.svg"), "<svg></svg>").unwrap();
    fs::write(
        root.join("pages/features/page1/assets/data.json"),
        r#"{"ok":true}"#,
    )
    .unwrap();

    render_site(
        root,
        RenderPlan {
            posts: false,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let not_found = fs::read_to_string(root.join("html/404.html")).unwrap();
    assert!(not_found.contains("Missing"));

    let about = fs::read_to_string(root.join("html/about/index.html")).unwrap();
    assert!(about.contains("About"));

    let feature = fs::read_to_string(root.join("html/features/page1/index.html")).unwrap();
    assert!(feature.contains("Feature"));
    assert_eq!(
        fs::read_to_string(root.join("html/features/page1/diagram.svg")).unwrap(),
        "<svg></svg>"
    );
    assert_eq!(
        fs::read_to_string(root.join("html/features/page1/assets/data.json")).unwrap(),
        r#"{"ok":true}"#
    );
}

#[test]
fn writes_search_index_with_posts() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    setup_markdown_templates(root);
    write_markdown_post(
        root,
        "This example body contains enough English text to exercise the search index.",
    );

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let index_path = root.join("html/assets/search/search-index.json");
    assert!(index_path.exists());
    let data = fs::read_to_string(index_path).unwrap();
    let payload: serde_json::Value = serde_json::from_str(&data).unwrap();
    assert_eq!(payload["documents"].as_array().unwrap().len(), 1);
    assert_eq!(payload["documents"][0]["language"], "en");
}

#[test]
fn search_index_updates_when_post_changes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    setup_markdown_templates(root);
    write_markdown_post(
        root,
        "Initial body content with enough characters for indexing.",
    );

    let full_plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Full,
        verbose: false,
    };
    render_site(root, full_plan).unwrap();

    let index_path = root.join("html/assets/search/search-index.json");
    let original = fs::read_to_string(&index_path).unwrap();

    fs::write(
            root.join("posts/hello-world/post.md"),
            "---\ntitle: Example\ndate: 2024-01-02T03:04:05Z\ntags: [test]\n---\nChanged body text that modifies the search index.",
        )
        .unwrap();

    let changed_plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Changed,
        verbose: false,
    };
    render_site(root, changed_plan).unwrap();

    let updated = fs::read_to_string(&index_path).unwrap();
    assert_ne!(original, updated);
}

#[test]
fn search_index_not_rewritten_when_unchanged() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    setup_markdown_templates(root);
    write_markdown_post(
        root,
        "Stable body content with enough characters for indexing.",
    );

    let full_plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Full,
        verbose: false,
    };
    render_site(root, full_plan).unwrap();

    let index_path = root.join("html/assets/search/search-index.json");
    let original_mtime = file_mtime(&index_path);

    wait_for_filesystem_tick();

    let changed_plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Changed,
        verbose: false,
    };
    render_site(root, changed_plan).unwrap();

    assert_eq!(original_mtime, file_mtime(&index_path));
}

#[test]
fn attachment_serialization_is_deterministically_sorted() {
    use crate::content::discover_posts;

    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let post_dir = root.join("posts/attach");
    fs::create_dir_all(&post_dir).unwrap();
    // Front-matter lists attachments in reverse-sorted order; the serialized
    // summary must still emit them sorted so the cache digest is stable across
    // renders (a HashMap would order by a per-process-random seed).
    fs::write(
        post_dir.join("post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\nattached: [b.txt, a.txt]\n---\nBody",
    )
    .unwrap();
    fs::write(post_dir.join("a.txt"), "a").unwrap();
    fs::write(post_dir.join("b.txt"), "b").unwrap();

    let config = crate::config::Config::default();
    let posts = discover_posts(root.join("posts"), &config, None).unwrap();
    let summary = super::posts::build_post_summary(&config, &posts[0]).unwrap();

    let json = serde_json::to_string(&summary).unwrap();
    let a_pos = json.find("\"a.txt\"").expect("a.txt attachment present");
    let b_pos = json.find("\"b.txt\"").expect("b.txt attachment present");
    assert!(
        a_pos < b_pos,
        "attachments must serialize in sorted key order, got: {json}"
    );
}

#[test]
fn exposes_additional_front_matter_in_templates() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);

    fs::write(
            root.join("templates/post.html"),
            "{% extends \"base.html\" %}{% block content %}<article>{{ post.location.country }}</article>{% endblock %}",
        )
        .unwrap();

    fs::create_dir_all(root.join("posts/location")).unwrap();
    fs::write(
        root.join("posts/location/post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\nlocation:\n  country: GR\n---\nBody",
    )
    .unwrap();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let rendered = fs::read_to_string(root.join("html/2024/01/01/location/index.html")).unwrap();
    assert!(rendered.contains("GR"));
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
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let copied = root.join("html/css/site.css");
    assert!(copied.exists());
}

#[test]
fn static_assets_recopied_when_file_changes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("skel")).unwrap();
    fs::write(root.join("skel/style.css"), "body { color: black; }").unwrap();
    setup_markdown_templates(root);
    // A post is required so the site-inputs hash is persisted; without it every
    // render sees "site changed" and forces a Full rebuild, which always copies
    // and would bypass the static digest gate this test exercises.
    write_markdown_post(root, "Body for the static-assets test.");

    let full_plan = RenderPlan {
        posts: true,
        static_assets: true,
        mode: BuildMode::Full,
        verbose: false,
    };
    render_site(root, full_plan).unwrap();

    let copied = root.join("html/style.css");
    assert_eq!(
        fs::read_to_string(&copied).unwrap(),
        "body { color: black; }"
    );

    wait_for_filesystem_tick();

    // Different content and length so the metadata-only digest still changes.
    fs::write(
        root.join("skel/style.css"),
        "body { color: rebeccapurple; font-size: 16px; }",
    )
    .unwrap();
    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: true,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(&copied).unwrap(),
        "body { color: rebeccapurple; font-size: 16px; }"
    );
}

#[test]
fn static_assets_skipped_when_unchanged() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("skel")).unwrap();
    fs::write(root.join("skel/style.css"), "body { color: black; }").unwrap();
    setup_markdown_templates(root);
    // A post is required so the site-inputs hash is persisted; without it the
    // incremental render is forced to Full and re-copies unconditionally.
    write_markdown_post(root, "Body for the static-assets test.");

    let full_plan = RenderPlan {
        posts: true,
        static_assets: true,
        mode: BuildMode::Full,
        verbose: false,
    };
    render_site(root, full_plan).unwrap();

    let copied = root.join("html/style.css");
    let copied_mtime = file_mtime(&copied);

    wait_for_filesystem_tick();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: true,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    assert_eq!(copied_mtime, file_mtime(&copied));
}

#[test]
fn paginates_homepage_with_page_numbers() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);
    fs::write(root.join("bckt.yaml"), "homepage_posts: 1\n").unwrap();

    write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "A");
    write_dated_post(root, "beta", "2024-02-01T00:00:00Z", "B");
    write_dated_post(root, "gamma", "2024-03-01T00:00:00Z", "C");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    // Posts are sorted ascending, so page 1 has alpha (oldest), homepage has gamma (newest)
    // Homepage is at the end of the pagination sequence, so prev goes backward to page 2
    let index = fs::read_to_string(root.join("html/index.html")).unwrap();
    assert!(index.contains("article data-slug=\"gamma\""));
    assert!(index.contains("data-prev=\"/page/2/\""));
    assert!(index.contains("data-next=\"\""));
    assert!(index.contains("data-current=\"3\""));
    assert!(index.contains("data-total=\"3\""));

    // Page 2 is in the middle
    let second = fs::read_to_string(root.join("html/page/2/index.html")).unwrap();
    assert!(second.contains("article data-slug=\"beta\""));
    assert!(second.contains("data-prev=\"/page/1/\""));
    assert!(second.contains("data-next=\"/\""));
    assert!(second.contains("data-current=\"2\""));
    assert!(second.contains("data-total=\"3\""));

    // Page 1 is at the beginning
    let first = fs::read_to_string(root.join("html/page/1/index.html")).unwrap();
    assert!(first.contains("article data-slug=\"alpha\""));
    assert!(first.contains("data-prev=\"\""));
    assert!(first.contains("data-next=\"/page/2/\""));
    assert!(first.contains("data-current=\"1\""));
    assert!(first.contains("data-total=\"3\""));

    // Add a new post and ensure homepage is updated but old pages remain stable
    write_dated_post(root, "delta", "2024-04-01T00:00:00Z", "D");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    // Homepage now shows delta (newest), prev goes to page 3
    let refreshed_index = fs::read_to_string(root.join("html/index.html")).unwrap();
    assert!(refreshed_index.contains("article data-slug=\"delta\""));
    assert!(refreshed_index.contains("data-prev=\"/page/3/\""));
    assert!(refreshed_index.contains("data-current=\"4\""));
    assert!(refreshed_index.contains("data-total=\"4\""));

    // Page 1 (alpha) and Page 2 (beta) should still exist and be unchanged
    assert!(root.join("html/page/1/index.html").exists());
    assert!(root.join("html/page/2/index.html").exists());
}

#[test]
fn renders_tag_pages_without_pagination() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);
    fs::write(
        root.join("bckt.yaml"),
        "homepage_posts: 5\npaginate_tags: false\n",
    )
    .unwrap();

    write_tagged_post(root, "first", "shared", "2024-01-01T00:00:00Z", "Body A");
    write_tagged_post(root, "second", "shared", "2024-02-01T00:00:00Z", "Body B");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let tag_root = root.join("html/tags/shared");
    assert!(tag_root.join("index.html").exists());
    assert!(!tag_root.join("first").exists());
}

#[test]
fn renders_tag_pages_with_pagination() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);
    fs::write(
        root.join("bckt.yaml"),
        "homepage_posts: 1\npaginate_tags: true\n",
    )
    .unwrap();

    write_tagged_post(root, "alpha", "shared", "2024-01-01T00:00:00Z", "A");
    write_tagged_post(root, "beta", "shared", "2024-02-01T00:00:00Z", "B");
    write_tagged_post(root, "gamma", "shared", "2024-03-01T00:00:00Z", "C");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let tag_index = fs::read_to_string(root.join("html/tags/shared/index.html")).unwrap();
    assert!(tag_index.contains("article data-slug=\"gamma\""));
    assert!(tag_index.contains("article data-slug=\"beta\""));
    assert!(tag_index.contains("article data-slug=\"alpha\""));
    assert!(tag_index.contains("data-total=\"1\""));
    assert!(tag_index.contains("data-prev=\"\""));
    assert!(tag_index.contains("data-next=\"\""));

    assert!(!root.join("html/tags/shared/gamma").exists());
    assert!(!root.join("html/tags/shared/beta").exists());
    assert!(!root.join("html/tags/shared/alpha").exists());
}

#[test]
fn generates_rss_feed_with_absolute_urls() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);
    fs::write(
        root.join("bckt.yaml"),
        "base_url: \"https://example.com/blog\"\n",
    )
    .unwrap();

    write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "Alpha body");
    write_dated_post(root, "beta", "2024-02-01T00:00:00Z", "Beta body");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let feed = fs::read_to_string(root.join("html/rss.xml")).unwrap();
    assert!(feed.contains("<link>https://example.com/blog/</link>"));
    assert!(feed.contains("<atom:link href=\"https://example.com/blog/rss.xml\""));
    assert!(feed.contains("<link>https://example.com/blog/2024/02/01/beta/</link>"));
    assert!(feed.contains("<description>Beta body"));
    assert!(feed.contains("<content:encoded><![CDATA["));
}

#[test]
fn generates_tag_rss_feeds_when_configured() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);
    fs::write(
        root.join("bckt.yaml"),
        "title: Demo Site\nbase_url: \"https://example.com\"\nrss_tags:\n  - shared\n",
    )
    .unwrap();

    write_tagged_post(root, "alpha", "shared", "2024-01-01T00:00:00Z", "A");
    write_tagged_post(root, "beta", "other", "2024-02-01T00:00:00Z", "B");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let feed_path = root.join("html/rss-shared.xml");
    assert!(feed_path.exists());
    let feed = fs::read_to_string(feed_path).unwrap();
    assert!(feed.contains("shared · Demo Site"));
    assert!(feed.contains("/2024/01/01/alpha/"));
    assert!(!feed.contains("/2024/02/01/beta/"));
}

#[test]
fn rss_not_rewritten_when_unchanged() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    setup_markdown_templates(root);
    write_markdown_post(root, "Stable feed body content for the RSS feed.");

    let full_plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Full,
        verbose: false,
    };
    render_site(root, full_plan).unwrap();

    let rss_path = root.join("html/rss.xml");
    let sitemap_path = root.join("html/sitemap.xml");
    let rss_mtime = file_mtime(&rss_path);
    let sitemap_mtime = file_mtime(&sitemap_path);

    wait_for_filesystem_tick();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    assert_eq!(rss_mtime, file_mtime(&rss_path));
    assert_eq!(sitemap_mtime, file_mtime(&sitemap_path));
}

#[test]
fn rss_rewritten_when_post_changes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    setup_markdown_templates(root);
    write_markdown_post(root, "Original feed body content.");

    let full_plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Full,
        verbose: false,
    };
    render_site(root, full_plan).unwrap();

    let rss_path = root.join("html/rss.xml");
    let rss_mtime = file_mtime(&rss_path);

    wait_for_filesystem_tick();

    fs::write(
        root.join("posts/hello-world/post.md"),
        "---\ntitle: Example\ndate: 2024-01-02T03:04:05Z\ntags: [test]\n---\nChanged feed body content that alters the RSS output.",
    )
    .unwrap();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    assert_ne!(rss_mtime, file_mtime(&rss_path));
}

#[test]
fn stale_tag_feed_removed_when_config_drops_tag() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);
    fs::write(
        root.join("bckt.yaml"),
        "title: Demo Site\nbase_url: \"https://example.com\"\nrss_tags:\n  - shared\n",
    )
    .unwrap();
    write_tagged_post(root, "alpha", "shared", "2024-01-01T00:00:00Z", "A");

    let plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Changed,
        verbose: false,
    };
    render_site(root, plan).unwrap();

    let tag_feed = root.join("html/rss-shared.xml");
    assert!(tag_feed.exists());

    // Drop the tag from config; the config change forces a full rebuild and the
    // now-unconfigured tag feed must be cleaned up.
    fs::write(
        root.join("bckt.yaml"),
        "title: Demo Site\nbase_url: \"https://example.com\"\n",
    )
    .unwrap();
    render_site(root, plan).unwrap();

    assert!(!tag_feed.exists());
}

#[test]
fn keeps_relative_paths_in_html_and_absolute_in_feeds() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts/media/images")).unwrap();
    setup_markdown_templates(root);
    fs::write(root.join("posts/media/images/pic.png"), "image-bytes").unwrap();
    fs::write(root.join("posts/media/notes.txt"), "notes").unwrap();
    fs::write(
            root.join("posts/media/post.md"),
            "---\ndate: 2024-01-01T00:00:00Z\nattached:\n  - images/pic.png\n  - notes.txt\n---\n![Alt](images/pic.png)\n\n[Download](notes.txt)\n",
        )
        .unwrap();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let post_page = fs::read_to_string(root.join("html/2024/01/01/media/index.html")).unwrap();
    // HTML pages use relative paths (works regardless of base_url)
    assert!(post_page.contains("images/pic.png"));
    assert!(post_page.contains("notes.txt"));
    // Should not contain absolute paths
    assert!(!post_page.contains("/2024/01/01/media/images/pic.png"));
    assert!(!post_page.contains("/2024/01/01/media/notes.txt"));

    let feed = fs::read_to_string(root.join("html/rss.xml")).unwrap();
    // RSS feeds use absolute URLs (required for feed readers)
    assert!(feed.contains("/2024/01/01/media/images/pic.png"));
    assert!(feed.contains("/2024/01/01/media/notes.txt"));
}

#[test]
fn generates_sitemap_with_posts_tags_and_pages() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);
    fs::write(
        root.join("bckt.yaml"),
        "base_url: \"https://example.com/blog\"\nhomepage_posts: 1\npaginate_tags: true\n",
    )
    .unwrap();

    write_tagged_post(root, "alpha", "shared", "2024-01-01T00:00:00Z", "A");
    write_tagged_post(root, "beta", "shared", "2024-02-01T00:00:00Z", "B");
    write_tagged_post(root, "gamma", "shared", "2024-03-01T00:00:00Z", "C");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let sitemap = fs::read_to_string(root.join("html/sitemap.xml")).unwrap();
    assert!(sitemap.contains("<loc>https://example.com/blog/</loc>"));

    // Page-number based URLs (page 1 = oldest, page 2 = middle)
    assert!(sitemap.contains("<loc>https://example.com/blog/page/1/</loc>"));
    assert!(sitemap.contains("<loc>https://example.com/blog/page/2/</loc>"));
    assert!(sitemap.contains("<loc>https://example.com/blog/tags/shared/</loc>"));
    assert!(sitemap.contains("<loc>https://example.com/blog/2024/03/01/gamma/</loc>"));
    assert!(sitemap.contains("<lastmod>2024-03-01T00:00:00Z</lastmod>"));
}

#[test]
fn skips_rewriting_tag_index_when_unchanged() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);

    write_tagged_post(root, "alpha", "shared", "2024-01-01T00:00:00Z", "A");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let tag_path = root.join("html/tags/shared/index.html");
    assert!(tag_path.exists());
    let first_mtime = file_mtime(&tag_path);

    wait_for_filesystem_tick();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    let second_mtime = file_mtime(&tag_path);
    assert_eq!(first_mtime, second_mtime);
}

#[test]
fn rerenders_tag_index_when_post_changes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);

    write_tagged_post(root, "alpha", "shared", "2024-01-01T00:00:00Z", "A");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let tag_path = root.join("html/tags/shared/index.html");
    let first_mtime = file_mtime(&tag_path);

    wait_for_filesystem_tick();

    fs::write(
            root.join("posts/alpha/post.md"),
            "---\ntitle: Alpha Updated\ndate: 2024-01-01T00:00:00Z\nslug: alpha\ntags:\n  - shared\n---\nUpdated",
        )
        .unwrap();

    wait_for_filesystem_tick();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    let second_mtime = file_mtime(&tag_path);
    assert!(second_mtime > first_mtime);
}

#[test]
fn homepage_rerenders_when_post_body_changes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);

    write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "A");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let index_path = root.join("html/index.html");
    let first_mtime = file_mtime(&index_path);

    wait_for_filesystem_tick();

    write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "Updated body");

    wait_for_filesystem_tick();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    let second_mtime = file_mtime(&index_path);
    assert!(second_mtime > first_mtime);
}

#[test]
fn removes_tag_index_when_tag_disappears() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);

    write_tagged_post(root, "alpha", "shared", "2024-01-01T00:00:00Z", "A");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let tag_path = root.join("html/tags/shared/index.html");
    assert!(tag_path.exists());

    wait_for_filesystem_tick();

    fs::remove_dir_all(root.join("posts/alpha")).unwrap();

    wait_for_filesystem_tick();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    assert!(!tag_path.exists());
}

#[test]
fn skips_rewriting_archives_when_unchanged() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);

    write_dated_post(root, "alpha", "2024-02-01T00:00:00Z", "A");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let year_path = root.join("html/2024/index.html");
    let month_path = root.join("html/2024/02/index.html");
    let first_year_mtime = file_mtime(&year_path);
    let first_month_mtime = file_mtime(&month_path);

    wait_for_filesystem_tick();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    let second_year_mtime = file_mtime(&year_path);
    let second_month_mtime = file_mtime(&month_path);

    assert_eq!(first_year_mtime, second_year_mtime);
    assert_eq!(first_month_mtime, second_month_mtime);
}

#[test]
fn rerenders_archives_when_post_changes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);

    write_dated_post(root, "alpha", "2024-03-01T00:00:00Z", "Original");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let year_path = root.join("html/2024/index.html");
    let month_path = root.join("html/2024/03/index.html");
    let first_year_mtime = file_mtime(&year_path);
    let first_month_mtime = file_mtime(&month_path);

    wait_for_filesystem_tick();

    fs::write(
            root.join("posts/alpha/post.md"),
            "---\ntitle: Alpha\ndate: 2024-03-01T00:00:00Z\nslug: alpha\ntags:\n  - alpha\n---\nUpdated body",
        )
        .unwrap();

    wait_for_filesystem_tick();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    let second_year_mtime = file_mtime(&year_path);
    let second_month_mtime = file_mtime(&month_path);

    assert!(second_year_mtime > first_year_mtime);
    assert!(second_month_mtime > first_month_mtime);
}

#[test]
fn removes_archives_when_posts_are_removed() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);

    write_dated_post(root, "alpha", "2024-04-01T00:00:00Z", "Body");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let year_path = root.join("html/2024/index.html");
    let month_path = root.join("html/2024/04/index.html");
    assert!(year_path.exists());
    assert!(month_path.exists());

    wait_for_filesystem_tick();

    fs::remove_dir_all(root.join("posts/alpha")).unwrap();

    wait_for_filesystem_tick();

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Changed,
            verbose: false,
        },
    )
    .unwrap();

    assert!(!year_path.exists());
    assert!(!month_path.exists());
}

#[test]
fn renders_year_and_month_archives() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);

    write_dated_post(root, "jan", "2023-01-01T00:00:00Z", "Old");
    write_dated_post(root, "feb", "2024-02-01T00:00:00Z", "Mid");
    write_dated_post(root, "mar", "2024-03-01T00:00:00Z", "New");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    assert!(root.join("html/2024/index.html").exists());
    assert!(root.join("html/2024/03/index.html").exists());
    assert!(root.join("html/2023/index.html").exists());
}

#[test]
fn incremental_rebuilds_only_changed_post() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    setup_markdown_templates(root);

    write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "Alpha body");
    write_dated_post(root, "beta", "2024-02-01T00:00:00Z", "Beta body");

    let alpha_output = root.join("html/2024/01/01/alpha/index.html");
    let beta_output = root.join("html/2024/02/01/beta/index.html");

    let full_plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Full,
        verbose: false,
    };
    let changed_plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Changed,
        verbose: false,
    };

    render_site(root, full_plan).unwrap();

    let alpha_first = file_mtime(&alpha_output);
    let beta_first = file_mtime(&beta_output);

    wait_for_filesystem_tick();
    render_site(root, changed_plan).unwrap();

    let alpha_second = file_mtime(&alpha_output);
    let beta_second = file_mtime(&beta_output);
    assert_eq!(alpha_first, alpha_second);
    assert_eq!(beta_first, beta_second);

    wait_for_filesystem_tick();
    write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "Alpha updated");
    render_site(root, changed_plan).unwrap();

    let alpha_third = file_mtime(&alpha_output);
    let beta_third = file_mtime(&beta_output);
    assert!(alpha_third > alpha_second);
    assert_eq!(beta_second, beta_third);
}

#[test]
fn template_change_triggers_full_rebuild() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    setup_markdown_templates(root);

    write_dated_post(root, "alpha", "2024-01-01T00:00:00Z", "Alpha body");
    write_dated_post(root, "beta", "2024-02-01T00:00:00Z", "Beta body");

    let alpha_output = root.join("html/2024/01/01/alpha/index.html");
    let beta_output = root.join("html/2024/02/01/beta/index.html");

    let full_plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Full,
        verbose: false,
    };
    let changed_plan = RenderPlan {
        posts: true,
        static_assets: false,
        mode: BuildMode::Changed,
        verbose: false,
    };

    render_site(root, full_plan).unwrap();
    let alpha_initial = file_mtime(&alpha_output);
    let beta_initial = file_mtime(&beta_output);

    wait_for_filesystem_tick();
    render_site(root, changed_plan).unwrap();
    let alpha_after_changed = file_mtime(&alpha_output);
    let beta_after_changed = file_mtime(&beta_output);
    assert_eq!(alpha_initial, alpha_after_changed);
    assert_eq!(beta_initial, beta_after_changed);

    wait_for_filesystem_tick();
    write_template(
        root,
        "base.html",
        "<!doctype html><html><body data-version=\"v2\">{% block content %}{% endblock %}</body></html>",
    );
    render_site(root, changed_plan).unwrap();

    let alpha_after_template = file_mtime(&alpha_output);
    let beta_after_template = file_mtime(&beta_output);
    assert!(alpha_after_template > alpha_after_changed);
    assert!(beta_after_template > beta_after_changed);
}

#[test]
fn archive_years_global_available_in_all_templates() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("posts")).unwrap();
    setup_markdown_templates(root);

    // Override index.html to emit archive_years so we can assert on it.
    write_template(
        root,
        "index.html",
        "{% for y in archive_years %}{{ y.year }}:{{ y.count }} {% endfor %}",
    );

    write_dated_post(root, "post-2024", "2024-06-01T00:00:00Z", "A");
    write_dated_post(root, "post-2023", "2023-03-15T00:00:00Z", "B");

    render_site(
        root,
        RenderPlan {
            posts: true,
            static_assets: false,
            mode: BuildMode::Full,
            verbose: false,
        },
    )
    .unwrap();

    let index = fs::read_to_string(root.join("html/index.html")).unwrap();
    // Newest year first, with correct per-year counts.
    assert!(
        index.contains("2024:1"),
        "expected 2024:1 in index, got: {index}"
    );
    assert!(
        index.contains("2023:1"),
        "expected 2023:1 in index, got: {index}"
    );
    let pos_2024 = index.find("2024").unwrap();
    let pos_2023 = index.find("2023").unwrap();
    assert!(
        pos_2024 < pos_2023,
        "2024 should appear before 2023 (newest-first)"
    );
}
