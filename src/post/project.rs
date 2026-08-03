use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use time::OffsetDateTime;

use super::datetime::date_prefix;

/// Walks up from `start` until a directory containing `bckt.yaml` is found.
pub fn find_project_root(start: impl AsRef<Path>) -> Result<PathBuf> {
    let mut current = start.as_ref().to_path_buf();

    loop {
        if current.join("bckt.yaml").exists() {
            return Ok(current);
        }
        if !current.pop() {
            bail!(
                "could not locate bckt.yaml starting from {}",
                start.as_ref().display()
            );
        }
    }
}

/// Returns `<posts_root>/<YYYY>/<YYMMDD>-<slug>`, the conventional post directory.
pub fn post_dir(posts_root: &Path, date: &OffsetDateTime, slug: &str) -> PathBuf {
    posts_root
        .join(date.year().to_string())
        .join(format!("{}-{}", date_prefix(date), slug))
}

/// Returns the content file inside `post_dir`: `<slug>.md`.
pub fn post_file(post_dir: &Path, slug: &str) -> PathBuf {
    post_dir.join(format!("{}.md", slug))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use time::macros::datetime;

    #[test]
    fn post_dir_uses_year_and_date_prefix() {
        let dir = post_dir(
            Path::new("/site/posts"),
            &datetime!(2024-01-05 00:00:00 UTC),
            "hello",
        );
        assert_eq!(dir, Path::new("/site/posts/2024/240105-hello"));
    }

    #[test]
    fn post_file_appends_markdown_extension() {
        assert_eq!(
            post_file(Path::new("/site/posts/2024/240105-hello"), "hello"),
            Path::new("/site/posts/2024/240105-hello/hello.md")
        );
    }

    #[test]
    fn find_project_root_walks_upwards() {
        let dir = TempDir::new().unwrap();
        let project = dir.path();
        let nested = project.join("posts/example");
        fs::create_dir_all(&nested).unwrap();
        fs::write(project.join("bckt.yaml"), "title: test\n").unwrap();

        let discovered = find_project_root(&nested).unwrap();
        assert_eq!(discovered, project);
    }

    #[test]
    fn find_project_root_errors_when_missing() {
        let dir = TempDir::new().unwrap();
        let error = find_project_root(dir.path()).unwrap_err();
        assert!(error.to_string().contains("could not locate bckt.yaml"));
    }
}
