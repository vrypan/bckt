pub fn absolute_url(base: &str, path: &str) -> String {
    let trimmed_base = base.trim_end_matches('/');
    let trimmed_path = path.trim_start_matches('/');

    if trimmed_path.is_empty() {
        format!("{}/", trimmed_base)
    } else {
        format!("{}/{trimmed_path}", trimmed_base)
    }
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
}
