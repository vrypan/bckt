use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options, format_html, parse_document};

const EXCERPT_LIMIT: usize = 280;

pub struct MarkdownRender {
    pub html: String,
    pub excerpt: String,
}

pub fn render_markdown(markdown: &str) -> MarkdownRender {
    let options = options();
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &options);

    let excerpt = extract_excerpt(root, EXCERPT_LIMIT);

    let mut html = String::new();
    format_html(root, &options, &mut html).expect("writing to String cannot fail");

    MarkdownRender { html, excerpt }
}

fn options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.footnotes = true;
    options.extension.alerts = true;
    options.extension.shortcodes = true;
    options.render.hardbreaks = false;
    options.render.github_pre_lang = true;
    options.render.r#unsafe = true;
    options.render.figure_with_caption = true;
    options.render.width = 0;
    options
}

fn extract_excerpt<'a>(root: &'a AstNode<'a>, limit: usize) -> String {
    if let Some(paragraph) = root
        .children()
        .find(|node| matches!(node.data.borrow().value, NodeValue::Paragraph))
    {
        return truncate(&collect_text(paragraph), limit);
    }

    truncate(&collect_text(root), limit)
}

fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut text = String::new();
    collect(node, &mut text);
    text.trim().to_string()
}

fn collect<'a>(node: &'a AstNode<'a>, buf: &mut String) {
    use NodeValue::*;
    let value = node.data.borrow();
    match &value.value {
        Text(literal) => buf.push_str(literal),
        Code(code) => buf.push_str(&code.literal),
        SoftBreak | LineBreak => buf.push(' '),
        CodeBlock(code) => {
            buf.push_str(&code.literal);
            buf.push(' ');
        }
        _ => {
            for child in node.children() {
                collect(child, buf);
            }
        }
    }
}

fn truncate(text: &str, limit: usize) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut result = String::new();
    let mut count = 0;
    let total = text.chars().count();
    for ch in text.chars() {
        if count >= limit {
            break;
        }
        result.push(ch);
        count += 1;
    }
    if total > count {
        result.push_str("...");
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_tables_and_tasks() {
        let markdown = "| h1 | h2 |\n| -- | -- |\n| a | b |\n\n- [x] done\n- [ ] todo";
        let rendered = render_markdown(markdown);
        assert!(rendered.html.contains("<table"), "{}", rendered.html);
        assert!(
            rendered.html.contains("<input type=\"checkbox\""),
            "{}",
            rendered.html
        );
    }

    #[test]
    fn renders_footnotes_and_code() {
        let markdown =
            "Paragraph with footnote.[^1]\n\n[^1]: Footnote text\n\n```rust\nfn main() {}\n```";
        let rendered = render_markdown(markdown);
        assert!(
            rendered.html.contains("data-footnotes"),
            "{}",
            rendered.html
        );
        assert!(rendered.html.contains("lang=\"rust\""), "{}", rendered.html);
    }

    #[test]
    fn excerpt_prefers_first_paragraph() {
        let markdown = "First paragraph.\n\nSecond paragraph";
        let rendered = render_markdown(markdown);
        assert_eq!(rendered.excerpt, "First paragraph.");
    }

    #[test]
    fn excerpt_truncates_long_text() {
        let text = "a".repeat(500);
        let rendered = render_markdown(&text);
        assert_eq!(rendered.excerpt.len(), EXCERPT_LIMIT + 3);
        assert!(rendered.excerpt.ends_with("..."));
    }

    #[test]
    fn renders_github_alerts() {
        let markdown = "> [!NOTE]\n> This is a note alert\n\n> [!WARNING]\n> This is a warning";
        let rendered = render_markdown(markdown);
        assert!(
            rendered.html.contains("markdown-alert"),
            "{}",
            rendered.html
        );
        assert!(
            rendered.html.contains("markdown-alert-note"),
            "{}",
            rendered.html
        );
        assert!(
            rendered.html.contains("markdown-alert-warning"),
            "{}",
            rendered.html
        );
    }

    #[test]
    fn renders_emoji_shortcodes() {
        let markdown = "Hello :smile: and :heart: world!";
        let rendered = render_markdown(markdown);
        assert!(rendered.html.contains("😄"), "{}", rendered.html);
        assert!(rendered.html.contains("❤"), "{}", rendered.html);
    }

    #[test]
    fn renders_figure_with_caption() {
        let markdown = "![alt text](https://example.com/image.png \"Image caption\")";
        let rendered = render_markdown(markdown);
        assert!(rendered.html.contains("<figure>"), "{}", rendered.html);
        assert!(rendered.html.contains("<figcaption>"), "{}", rendered.html);
        assert!(rendered.html.contains("Image caption"), "{}", rendered.html);
    }
}
