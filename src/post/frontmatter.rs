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
        fm.push_str(&format!("slug: {}\n", self.slug));
        fm.push_str(&format!("date: {}\n", yaml_quote(&self.date)));
        if !self.tags.is_empty() {
            fm.push_str(&format!("tags: {}\n", self.tags.join(", ")));
        }
        if let Some(pt) = self.post_type.as_deref().filter(|pt| !pt.trim().is_empty()) {
            fm.push_str(&format!("type: {}\n", pt.trim()));
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
            fm.push_str(&format!("language: {}\n", lang.trim()));
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

/// Wraps a value in double quotes, escaping backslashes and quotes.
pub fn yaml_quote(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
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
        assert!(post.render().contains("tags: rust, notes"), "{post:?}");
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
}
