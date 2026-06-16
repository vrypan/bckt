use anyhow::Result;
use std::env;
use std::path::{Path, PathBuf};

pub(crate) fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    slug
}

pub fn extract_base_path(base_url: &str) -> String {
    if let Some(idx) = base_url.find("://") {
        let after_scheme = &base_url[idx + 3..];
        if let Some(slash_idx) = after_scheme.find('/') {
            after_scheme[slash_idx..].trim_end_matches('/').to_string()
        } else {
            String::new()
        }
    } else {
        base_url.trim_end_matches('/').to_string()
    }
}

pub fn split_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

pub fn absolute_url(base: &str, path: &str) -> String {
    let trimmed_base = base.trim_end_matches('/');
    let trimmed_path = path.trim_start_matches('/');

    if trimmed_path.is_empty() {
        format!("{}/", trimmed_base)
    } else {
        format!("{}/{trimmed_path}", trimmed_base)
    }
}

/// Resolves a root path, expanding tilde and converting to absolute path.
/// If root_opt is None, returns the current working directory.
pub fn resolve_root(root_opt: Option<&str>) -> Result<PathBuf> {
    let path_str = root_opt.unwrap_or(".");
    let expanded = expand_tilde(path_str);
    let path = Path::new(&expanded);

    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let cwd = env::current_dir()?;
        Ok(cwd.join(path))
    }
}

/// Expands ~ to the user's home directory
fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/")
        && let Some(home) = env::var_os("HOME")
    {
        let home_str = home.to_string_lossy();
        return path.replacen("~", &home_str, 1);
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_root_path() {
        let url = absolute_url("https://example.com", "/");
        assert_eq!(url, "https://example.com/");
    }

    #[test]
    fn joins_nested_path() {
        let url = absolute_url("https://example.com/blog", "/rss.xml");
        assert_eq!(url, "https://example.com/blog/rss.xml");
    }

    #[test]
    fn trims_trailing_slash() {
        let url = absolute_url("https://example.com/", "/page/2/");
        assert_eq!(url, "https://example.com/page/2/");
    }

    #[test]
    fn extract_base_path_from_full_url() {
        assert_eq!(extract_base_path("https://vrypan.net/blog/"), "/blog");
        assert_eq!(extract_base_path("https://vrypan.net/blog"), "/blog");
        assert_eq!(extract_base_path("https://example.com/foo/bar/"), "/foo/bar");
    }

    #[test]
    fn extract_base_path_from_root_url() {
        assert_eq!(extract_base_path("https://vrypan.net/"), "");
        assert_eq!(extract_base_path("https://vrypan.net"), "");
    }

    #[test]
    fn extract_base_path_from_path_only() {
        assert_eq!(extract_base_path("/blog/"), "/blog");
        assert_eq!(extract_base_path("/blog"), "/blog");
    }

    #[test]
    fn split_csv_basic() {
        assert_eq!(split_csv("a, b, c"), vec!["a", "b", "c"]);
        assert_eq!(split_csv("rust"), vec!["rust"]);
        assert_eq!(split_csv(""), Vec::<String>::new());
        assert_eq!(split_csv(",, ,"), Vec::<String>::new());
    }

    #[test]
    fn resolve_root_handles_none() {
        let result = resolve_root(None).unwrap();
        assert_eq!(result, env::current_dir().unwrap());
    }

    #[test]
    fn resolve_root_handles_dot() {
        let result = resolve_root(Some(".")).unwrap();
        assert_eq!(result, env::current_dir().unwrap());
    }

    #[test]
    fn expand_tilde_expands_home() {
        let home = env::var("HOME").unwrap();
        let expanded = expand_tilde("~/test");
        assert_eq!(expanded, format!("{}/test", home));
    }

    #[test]
    fn expand_tilde_leaves_non_tilde_unchanged() {
        let expanded = expand_tilde("/absolute/path");
        assert_eq!(expanded, "/absolute/path");
    }
}
