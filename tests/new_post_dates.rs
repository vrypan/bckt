#![cfg(feature = "render")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use bckt::config::Config;
use bckt::content::discover_posts;
use bckt::post::{date_prefix, parse_datetime};
use tempfile::TempDir;

fn project() -> TempDir {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("bckt.yaml"), "title: Test\n").unwrap();
    temp
}

fn new_post(root: &Path, slug: &str, date: Option<&str>, extra: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bckt-new"));
    command
        .current_dir(root)
        .args(["--no-prompt", "--title", "Title", "--slug", slug])
        .args(extra);
    if let Some(date) = date {
        command.args(["--date", date]);
    }
    command.output().expect("failed to run bckt-new")
}

fn assert_rejected(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stdout: {stdout}");
    assert!(stderr.contains("invalid date"), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
    assert!(!stdout.contains("Created"), "stdout: {stdout}");
}

fn created_path(output: &Output) -> PathBuf {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = stdout
        .trim()
        .strip_prefix("Created new post at ")
        .unwrap_or_else(|| panic!("unexpected stdout: {stdout}"));
    PathBuf::from(path)
}

fn assert_post_dir(path: &Path, expected: PathBuf) {
    let actual = path.parent().unwrap().canonicalize().unwrap();
    assert_eq!(actual, expected.canonicalize().unwrap());
}

#[test]
fn new_post_invalid_date_writes_nothing() {
    for date in [
        "2024-02-30 10:00:00",
        "2024-13-01T00:00:00Z",
        "next tuesday",
    ] {
        let temp = project();

        let output = new_post(temp.path(), "bad-date", Some(date), &[]);

        assert_rejected(&output);
        assert!(!temp.path().join("posts").exists(), "{date}");
    }
}

#[test]
fn new_post_unicode_offset_writes_nothing() {
    for date in ["2024-01-15 12:00:00 +€a", "2024-01-15 12:00:00 +1½0"] {
        let temp = project();
        let posts_dir = temp.path().join("missing/posts");

        let output = new_post(
            temp.path(),
            "bad-offset",
            Some(date),
            &["--posts-dir", posts_dir.to_str().unwrap()],
        );

        assert_rejected(&output);
        assert!(!temp.path().join("missing").exists(), "{date}");
        assert!(!temp.path().join("posts").exists(), "{date}");
    }
}

#[test]
fn new_post_valid_date_matches_directory() {
    let temp = project();
    let cases = [
        ("rfc", "2024-05-06T07:08:09Z"),
        ("naive", "2024-05-06 07:08:09"),
        ("colon-offset", "2024-05-06 23:30:00 +03:00"),
        ("compact-offset", "2024-05-06 23:30:00 +0300"),
        ("utc", "2024-05-06 07:08:09 UTC"),
        ("zulu", "2024-05-06 07:08:09 Z"),
    ];

    for (slug, date) in cases {
        let path = created_path(&new_post(temp.path(), slug, Some(date), &[]));
        let expected = parse_datetime(date).unwrap();
        assert_post_dir(
            &path,
            temp.path()
                .join("posts/2024")
                .join(format!("{}-{slug}", date_prefix(&expected))),
        );
        assert!(fs::read_to_string(&path).unwrap().contains(date), "{date}");
    }

    let posts = discover_posts(temp.path().join("posts"), &Config::default(), None).unwrap();
    assert_eq!(posts.len(), cases.len());
    for (slug, date) in cases {
        let post = posts.iter().find(|post| post.slug == slug).unwrap();
        assert_eq!(post.date, parse_datetime(date).unwrap(), "{date}");
    }
}

#[test]
fn new_post_default_date_is_valid() {
    let temp = project();

    let path = created_path(&new_post(temp.path(), "today", None, &[]));

    let posts = discover_posts(temp.path().join("posts"), &Config::default(), None).unwrap();
    assert_eq!(posts.len(), 1);
    let date = posts[0].date;
    let expected_dir = temp
        .path()
        .join("posts")
        .join(date.year().to_string())
        .join(format!("{}-today", date_prefix(&date)));
    assert_post_dir(&path, expected_dir);
}

#[test]
fn new_post_collision_preserves_existing_post() {
    let temp = project();
    let date = Some("2024-05-06T07:08:09Z");
    let path = created_path(&new_post(temp.path(), "same", date, &[]));
    fs::write(&path, "edited").unwrap();

    let output = new_post(temp.path(), "same", date, &[]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "edited");
}
