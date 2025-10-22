use super::*;
use crate::config::Config;
use std::path::PathBuf;
use tempfile::TempDir;
use time::UtcOffset;

#[test]
fn discover_single_markdown_post() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts");
    fs::create_dir_all(root.join("notes/hello-world")).unwrap();
    fs::write(
        root.join("notes/hello-world/post.md"),
        "---\ntitle: Hello\ndate: 2024-02-01T12:00:00Z\ntags: [rust]\n---\nBody",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(&root, &config).unwrap();
    assert_eq!(posts.len(), 1);
    let post = &posts[0];
    assert_eq!(post.slug, "hello-world");
    assert_eq!(post.tags, vec!["rust".to_string()]);
    assert_eq!(post.permalink, "/2024/02/01/hello-world/");
    assert_eq!(post.body_html, "<p>Body</p>\n");
    assert_eq!(post.excerpt, "Body");
}

#[test]
fn prefer_slug_from_front_matter() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts");
    fs::create_dir_all(root.join("mixed/Example")).unwrap();
    fs::write(
        root.join("mixed/Example/post.md"),
        "---\ndate: 2024-03-04T00:00:00Z\nslug: Custom Slug\n---\n",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(&root, &config).unwrap();
    assert_eq!(posts[0].slug, "custom-slug");
}

#[test]
fn parse_full_front_matter_payload() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/full");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ntitle: Sample\ndate: 2024-05-06T08:09:10Z\ntags:\n  - summary\n  - rust\nabstract: Short\nattached:\n  - files/data.csv\nimages:\n  - img.png\nvideo_url: https://example.com/video.mp4\nlocation:\n  country: GR\n---\nBody\n",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    let post = &posts[0];
    assert_eq!(post.title.as_deref(), Some("Sample"));
    assert_eq!(post.tags, vec!["summary".to_string(), "rust".to_string()]);
    assert_eq!(post.abstract_text.as_deref(), Some("Short"));
    assert_eq!(post.attached, vec![PathBuf::from("files/data.csv")]);
    assert_eq!(post.body_html, "<p>Body</p>\n");
    assert_eq!(post.excerpt, "Body");
    assert_eq!(
        post.extra
            .get("location")
            .and_then(|value| value.get("country")),
        Some(&JsonValue::String("GR".to_string()))
    );
    assert_eq!(
        post.extra.get("images"),
        Some(&JsonValue::Array(vec![JsonValue::String("img.png".into())]))
    );
    assert_eq!(
        post.extra.get("video_url"),
        Some(&JsonValue::String("https://example.com/video.mp4".into()))
    );
}

#[test]
fn reject_duplicate_main_files() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/dupe");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "---\ndate: 2024-01-01T00:00:00Z\n---\n").unwrap();
    fs::write(
        root.join("b.html"),
        "---\ndate: 2024-01-01T00:00:00Z\n---\n",
    )
    .unwrap();

    let config = Config::default();
    let error = discover_posts(root.parent().unwrap(), &config).unwrap_err();
    assert!(format!("{error}").contains("expected exactly one"));
}

#[test]
fn reject_missing_front_matter() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/missing");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("post.md"), "no front matter").unwrap();

    let config = Config::default();
    let error = discover_posts(root.parent().unwrap(), &config).unwrap_err();
    assert!(format!("{error}").contains("front matter"));
}

#[test]
fn allow_front_matter_only() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/solo");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\n---\n",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    assert_eq!(posts[0].body_html, "");
    assert_eq!(posts[0].excerpt, "");
}

#[test]
fn retains_additional_front_matter() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/extras");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\nlocation:\n  country: GR\n  city: Athens\n---\n",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    let value = posts[0]
        .extra
        .get("location")
        .and_then(|map| map.get("city"))
        .cloned();

    assert_eq!(value, Some(JsonValue::String("Athens".to_string())));
}

#[test]
fn parse_comma_separated_lists() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/list");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\ntags: one, two , three\nattached: file-a.txt, file-b.txt\nimages: img-a.png\n---\n",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    let post = &posts[0];

    assert_eq!(post.tags, vec!["one", "two", "three"]);
    assert_eq!(
        post.attached,
        vec![PathBuf::from("file-a.txt"), PathBuf::from("file-b.txt")]
    );
    assert_eq!(
        post.extra.get("images"),
        Some(&JsonValue::String("img-a.png".into()))
    );
}

#[test]
fn allows_empty_tags_field() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/empty-tags");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\ntags:\n---\nBody",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    assert!(posts[0].tags.is_empty());
}

#[test]
fn allows_empty_attached_field() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/empty-attached");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\nattached:\n---\nBody",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    assert!(posts[0].attached.is_empty());
}

#[test]
fn accepts_datetime_with_numeric_offset() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/offset");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ndate: 2013-01-18 00:25:24 +0200\n---\nBody",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    let post = &posts[0];
    assert_eq!(post.date.offset(), UtcOffset::from_hms(2, 0, 0).unwrap());
}

#[test]
fn accepts_naive_datetime_with_default_timezone() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/naive");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ndate: 2024-01-02 09:30:00\n---\nBody",
    )
    .unwrap();

    let config = Config {
        default_timezone: "+02:00".to_string(),
        ..Default::default()
    };

    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    let post = &posts[0];
    let offset = config.default_offset().unwrap();
    assert_eq!(post.date.offset(), offset);
    assert_eq!(post.date.hour(), 9);
    assert_eq!(post.date.minute(), 30);
    assert_eq!(post.excerpt, "Body");
}

#[test]
fn language_from_front_matter_is_normalized() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/lang");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\nlanguage: EL\n---\nΔοκιμαστικό κείμενο.",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    assert_eq!(posts[0].language, "el");
}

#[test]
fn language_is_detected_when_missing() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/detect");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\n---\nΑυτό είναι ένα παράδειγμα ελληνικού κειμένου για την ανίχνευση γλώσσας.",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    assert_eq!(posts[0].language, "el");
}

#[test]
fn short_content_falls_back_to_default_language() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/fallback");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.md"),
        "---\ndate: 2024-01-01T00:00:00Z\n---\nHi!",
    )
    .unwrap();

    let mut config = Config::default();
    config.search.default_language = "en".to_string();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    assert_eq!(posts[0].language, "en");
}

#[test]
fn slugify_directory_name() {
    assert_eq!(slugify("Hello World"), "hello-world");
    assert_eq!(slugify("  Multi   Spaces  "), "multi-spaces");
}

#[test]
fn html_posts_are_passthrough() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts/page");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("post.html"),
        "---\ndate: 2024-01-02T00:00:00Z\n---\n<p>Sunny</p>",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(root.parent().unwrap(), &config).unwrap();
    assert_eq!(posts[0].body_html, "<p>Sunny</p>");
    assert_eq!(posts[0].excerpt, "Sunny");
}

#[test]
fn ignores_directories_with_bcktignore() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("posts");

    // Create a normal post
    fs::create_dir_all(root.join("published")).unwrap();
    fs::write(
        root.join("published/post.md"),
        "---\ntitle: Published\ndate: 2024-01-01T00:00:00Z\n---\nPublished content",
    )
    .unwrap();

    // Create a post in an ignored directory
    fs::create_dir_all(root.join("drafts")).unwrap();
    fs::write(root.join("drafts/.bcktignore"), "").unwrap();
    fs::write(
        root.join("drafts/post.md"),
        "---\ntitle: Draft\ndate: 2024-01-02T00:00:00Z\n---\nDraft content",
    )
    .unwrap();

    // Create a post in a nested ignored directory
    fs::create_dir_all(root.join("archive/old")).unwrap();
    fs::write(root.join("archive/.bcktignore"), "").unwrap();
    fs::write(
        root.join("archive/old/post.md"),
        "---\ntitle: Old\ndate: 2024-01-03T00:00:00Z\n---\nOld content",
    )
    .unwrap();

    let config = Config::default();
    let posts = discover_posts(&root, &config).unwrap();

    // Only the published post should be discovered
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].slug, "published");
}
