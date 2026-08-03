/// Converts arbitrary text into a bckt slug.
///
/// The mapping is frozen: slugs become permalinks, tag URLs, and post
/// directory names, so changing it would silently re-key every published post.
pub fn slugify(value: &str) -> String {
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

/// Splits a comma-separated list into trimmed, non-empty values.
pub fn normalize_tags(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|raw| raw.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
}

/// Returns the trimmed value, or `None` when it is blank.
pub fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_lowercases_and_joins_words() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("  Multi   Spaces  "), "multi-spaces");
    }

    #[test]
    fn slugify_drops_leading_and_trailing_separators() {
        assert_eq!(slugify("--Hello--"), "hello");
        assert_eq!(slugify("!!!"), "");
    }

    #[test]
    fn slugify_collapses_runs_of_punctuation() {
        assert_eq!(slugify("a -- b"), "a-b");
        assert_eq!(slugify("Ünïcodé is dropped"), "n-cod-is-dropped");
    }

    #[test]
    fn normalize_tags_empty_input_yields_no_tags() {
        assert!(normalize_tags("").is_empty());
    }

    #[test]
    fn normalize_tags_parses_csv() {
        assert_eq!(normalize_tags("rust, notes"), vec!["rust", "notes"]);
    }

    #[test]
    fn normalize_tags_skips_blank_entries() {
        assert_eq!(normalize_tags(",, rust ,,"), vec!["rust"]);
    }

    #[test]
    fn non_empty_trims_and_rejects_blanks() {
        assert_eq!(non_empty("  note  "), Some("note".to_string()));
        assert_eq!(non_empty("   "), None);
    }
}
