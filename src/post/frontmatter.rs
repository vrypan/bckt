/// A post's YAML front matter block.
///
/// Field order is fixed so that generated posts stay diff-stable across tools.
#[derive(Debug, Clone, Default)]
pub struct FrontMatter {
    pub title: Option<String>,
    pub slug: String,
    pub date: String,
    pub tags: Vec<String>,
    pub post_type: Option<String>,
    pub abstract_text: Option<String>,
    pub language: Option<String>,
    pub attached: Vec<String>,
}

impl FrontMatter {
    pub fn new(slug: impl Into<String>, date: impl Into<String>) -> Self {
        Self {
            slug: slug.into(),
            date: date.into(),
            ..Self::default()
        }
    }

    /// Renders the block, delimiters included, ending with a trailing newline.
    pub fn render(&self) -> String {
        let mut fm = String::new();
        fm.push_str("---\n");
        if let Some(title) = &self.title {
            fm.push_str(&format!("title: {}\n", yaml_quote(title)));
        }
        fm.push_str(&format!("slug: {}\n", yaml_quote(&self.slug)));
        fm.push_str(&format!("date: {}\n", yaml_quote(&self.date)));
        if !self.tags.is_empty() {
            fm.push_str("tags:\n");
            for tag in &self.tags {
                fm.push_str(&format!("  - {}\n", yaml_quote(tag)));
            }
        }
        if let Some(pt) = self.post_type.as_deref().filter(|pt| !pt.trim().is_empty()) {
            fm.push_str(&format!("type: {}\n", yaml_quote(pt.trim())));
        }
        if let Some(summary) = self
            .abstract_text
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fm.push_str(&format!("abstract: {}\n", yaml_quote(summary.trim())));
        }
        if let Some(lang) = self
            .language
            .as_deref()
            .filter(|lang| !lang.trim().is_empty())
        {
            fm.push_str(&format!("language: {}\n", yaml_quote(lang.trim())));
        }
        if self.attached.is_empty() {
            fm.push_str("attached:\n");
        } else {
            fm.push_str("attached:\n");
            for path in &self.attached {
                fm.push_str(&format!("  - {}\n", yaml_quote(path)));
            }
        }
        fm.push_str("---\n");
        fm
    }

    /// Renders the block followed by a blank line and `body`.
    pub fn into_document(&self, body: &str) -> String {
        format!("{}\n{}", self.render(), body)
    }
}

/// Renders a YAML double-quoted scalar that parses back to exactly `value`.
/// Line breaks and non-printable characters use YAML escapes, so a parser
/// neither folds lines nor rejects the document.
pub fn yaml_quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for ch in value.chars() {
        match ch {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            '\u{85}' => quoted.push_str("\\N"),
            '\u{2028}' => quoted.push_str("\\L"),
            '\u{2029}' => quoted.push_str("\\P"),
            c if c.is_control() || matches!(c, '\u{FEFF}' | '\u{FFFE}' | '\u{FFFF}') => {
                quoted.push_str(&format!("\\u{:04X}", c as u32));
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> FrontMatter {
        FrontMatter::new("title", "2024-01-01T00:00:00Z")
    }

    #[test]
    fn front_matter_omits_tags_line_when_empty() {
        let fm = sample().render();
        assert!(
            !fm.contains("tags:"),
            "front matter must not emit a tags line when there are no tags:\n{fm}"
        );
    }

    #[test]
    fn front_matter_includes_supplied_tags() {
        let mut post = sample();
        post.tags = vec!["rust".to_string(), "notes".to_string()];
        let fm = post.render();
        assert!(fm.contains("tags:\n  - \"rust\"\n  - \"notes\"\n"), "{fm}");
    }

    #[test]
    fn front_matter_quotes_scalar_fields_in_order() {
        let mut post = FrontMatter::new("my-slug", "2024-01-01T00:00:00Z");
        post.title = Some("T".to_string());
        post.tags = vec!["a".to_string()];
        post.post_type = Some(" note ".to_string());
        post.abstract_text = Some(" sum ".to_string());
        post.language = Some(" en ".to_string());
        assert_eq!(
            post.render(),
            "---\ntitle: \"T\"\nslug: \"my-slug\"\ndate: \"2024-01-01T00:00:00Z\"\ntags:\n  - \"a\"\ntype: \"note\"\nabstract: \"sum\"\nlanguage: \"en\"\nattached:\n---\n"
        );
    }

    #[test]
    fn front_matter_omits_title_when_absent() {
        let fm = sample().render();
        assert!(!fm.contains("title:"), "{fm}");
    }

    #[test]
    fn front_matter_emits_empty_title_when_set_to_blank() {
        let mut post = sample();
        post.title = Some(String::new());
        assert!(post.render().contains("title: \"\""));
    }

    #[test]
    fn front_matter_emits_bare_attached_key_when_empty() {
        let fm = sample().render();
        assert!(fm.contains("attached:\n---\n"), "{fm}");
    }

    #[test]
    fn front_matter_lists_attachments() {
        let mut post = sample();
        post.attached = vec!["img-1.jpg".to_string(), "img-2.jpg".to_string()];
        let fm = post.render();
        assert!(
            fm.contains("attached:\n  - \"img-1.jpg\"\n  - \"img-2.jpg\"\n"),
            "{fm}"
        );
    }

    #[test]
    fn front_matter_skips_blank_optional_fields() {
        let mut post = sample();
        post.post_type = Some("  ".to_string());
        post.abstract_text = Some(String::new());
        post.language = Some("   ".to_string());
        let fm = post.render();
        assert!(!fm.contains("type:"), "{fm}");
        assert!(!fm.contains("abstract:"), "{fm}");
        assert!(!fm.contains("language:"), "{fm}");
    }

    #[test]
    fn into_document_separates_body_with_one_blank_line() {
        let doc = sample().into_document("Hello.\n");
        assert!(doc.ends_with("---\n\nHello.\n"), "{doc}");
    }

    #[test]
    fn yaml_quote_escapes_quotes_and_backslashes() {
        assert_eq!(yaml_quote(r#"a "b" \c"#), r#""a \"b\" \\c""#);
        assert_eq!(yaml_quote(""), "\"\"");
    }

    #[test]
    fn yaml_quote_escapes_line_breaks_and_controls() {
        assert_eq!(yaml_quote("a\nb\rc\td"), r#""a\nb\rc\td""#);
        assert_eq!(yaml_quote("\0\u{7F}\u{1B}"), r#""\u0000\u007F\u001B""#);
        assert_eq!(yaml_quote("\u{85}\u{2028}\u{2029}"), r#""\N\L\P""#);
        assert_eq!(yaml_quote("café — 日本"), "\"café — 日本\"");
    }

    #[cfg(feature = "render")]
    fn parse_block(fm: &FrontMatter) -> serde_yaml::Mapping {
        let rendered = fm.render();
        let inner = rendered
            .strip_prefix("---\n")
            .and_then(|rest| rest.strip_suffix("---\n"))
            .expect("delimited block");
        serde_yaml::from_str(inner).unwrap_or_else(|err| panic!("{err}\n{rendered}"))
    }

    #[cfg(feature = "render")]
    fn field<'a>(map: &'a serde_yaml::Mapping, key: &str) -> &'a serde_yaml::Value {
        map.get(key).unwrap_or_else(|| panic!("missing {key}"))
    }

    #[cfg(feature = "render")]
    #[test]
    fn frontmatter_yaml_special_strings_round_trip() {
        let values = [
            "null",
            "true",
            "123",
            "~",
            "a: b # c",
            "- item",
            "{x: 1}",
            "'single' \"double\" \\back",
            "line one\nline two\r\n\ttabbed",
            "ctl\0\u{7}\u{1B}\u{7F}\u{85}\u{2028}\u{2029}\u{FEFF}end",
            "café — 日本 🚀",
        ];
        for value in values {
            let mut post = FrontMatter::new(value, value);
            post.title = Some(value.to_string());
            post.post_type = Some(format!("x{value}x"));
            post.abstract_text = Some(format!("x{value}x"));
            post.language = Some(format!("x{value}x"));
            let map = parse_block(&post);
            for key in ["title", "slug", "date"] {
                assert_eq!(field(&map, key).as_str(), Some(value), "{key}: {value:?}");
            }
            for key in ["type", "abstract", "language"] {
                let expected = format!("x{value}x");
                assert_eq!(field(&map, key).as_str(), Some(expected.as_str()), "{key}");
            }
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn frontmatter_yaml_tags_preserve_elements() {
        let tags = [
            "a: b",
            "a,b",
            "#topic",
            "[brackets]",
            "\"quoted\"",
            "back\\slash",
        ];
        let mut post = sample();
        post.tags = tags.iter().map(|tag| tag.to_string()).collect();
        let map = parse_block(&post);
        let parsed: Vec<&str> = field(&map, "tags")
            .as_sequence()
            .unwrap()
            .iter()
            .map(|tag| tag.as_str().unwrap())
            .collect();
        assert_eq!(parsed, tags);
    }

    #[cfg(feature = "render")]
    #[test]
    fn frontmatter_yaml_optional_contract() {
        let mut post = sample();
        post.post_type = Some("  ".to_string());
        post.attached = vec!["a b.png".to_string(), "c:d.txt".to_string()];
        let map = parse_block(&post);
        assert!(map.get("title").is_none());
        assert!(map.get("tags").is_none());
        assert!(map.get("type").is_none());
        let attached: Vec<&str> = field(&map, "attached")
            .as_sequence()
            .unwrap()
            .iter()
            .map(|path| path.as_str().unwrap())
            .collect();
        assert_eq!(attached, ["a b.png", "c:d.txt"]);
        assert!(field(&parse_block(&sample()), "attached").is_null());
        assert!(post.into_document("Body\n").ends_with("---\n\nBody\n"));
    }
}
