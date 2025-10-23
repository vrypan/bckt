# Posts and Content Organization

Posts in `bckt` are stored in the `posts/` directory. Each post is typically a directory containing a Markdown or HTML file along with any attached assets.

## Post Structure

A typical post directory looks like:

```
posts/
└── hello-world/
    ├── post.md              # Main content file
    ├── images/
    │   └── cover.jpg        # Attached images
    └── documents/
        └── paper.pdf        # Attached documents
```

## Frontmatter

Posts use YAML frontmatter to define metadata:

```yaml
---
title: Hello World
date: 2024-01-15T12:00:00Z
slug: hello-world
tags:
  - introduction
  - demo
image: images/cover.jpg
attached:
  - images/cover.jpg
  - documents/paper.pdf
---

Your post content here...
```

### Required Fields
- `date` — ISO 8601 timestamp (e.g., `2024-01-15T12:00:00Z`)

### Optional Fields
- `title` — Post title (defaults to slug if not provided)
- `slug` — URL-friendly identifier (defaults to directory name)
- `tags` — Array of tag strings
- `attached` — Array of relative paths to files that should be copied with the post
- Any custom fields are preserved in the `extra` map and accessible in templates

## Ignoring Directories

You can prevent directories from being discovered and rendered by placing a `.bcktignore` file in them:

```
posts/
├── published/              # ✓ Will be rendered
│   └── post.md
├── drafts/                 # ✗ Will be ignored
│   ├── .bcktignore         # ← Add this file
│   └── work-in-progress.md
└── archive/                # ✗ Will be ignored (including subdirectories)
    ├── .bcktignore         # ← Add this file
    ├── 2020/
    │   └── old-post.md
    └── 2021/
        └── another-old.md
```

When a directory contains `.bcktignore`:
- The directory and all its subdirectories are skipped during discovery
- No posts from these directories will be rendered or included in feeds
- Useful for drafts, archives, templates, or any content you want to keep but not publish

The `.bcktignore` file can be empty—its mere presence is enough to exclude the directory.

## Attached Files

Files listed in the `attached` frontmatter field are:
1. Copied to the post's output directory during rendering
2. Made available in templates via the `attachments` map with metadata:
   - `size` — file size in bytes
   - `mime_type` — MIME type detected from file extension

See [templates.md](templates.md#attachment-metadata) for usage examples.

## Markdown Extensions

`bckt` uses [Comrak](https://github.com/kivikakk/comrak) for Markdown rendering with support for GitHub Flavored Markdown (GFM) and additional extensions.

### Enabled Features

#### GitHub Flavored Markdown (GFM)

**Tables**
```markdown
| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |
```

**Strikethrough**
```markdown
~~This text is crossed out~~
```

**Autolinks**
```markdown
Visit www.example.com or https://github.com
```

**Task Lists**
```markdown
- [x] Completed task
- [ ] Pending task
```

#### GitHub Alerts

Create styled callout boxes using the alert syntax:

```markdown
> [!NOTE]
> Useful information that users should know, even when skimming content.

> [!TIP]
> Helpful advice for doing things better or more easily.

> [!IMPORTANT]
> Key information users need to know to achieve their goal.

> [!WARNING]
> Urgent info that needs immediate user attention to avoid problems.

> [!CAUTION]
> Advises about risks or negative outcomes of certain actions.
```

Alerts are rendered with CSS classes (`markdown-alert`, `markdown-alert-note`, etc.) that you can style in your theme.

#### Footnotes

Reference-style footnotes with automatic numbering:

```markdown
Here is some text with a footnote.[^1]

More text with another footnote.[^note]

[^1]: This is the first footnote.
[^note]: Named footnotes are also supported.
```

#### Emoji Shortcodes

Use emoji shortcodes for easy emoji insertion:

```markdown
Hello :wave: I :heart: Markdown! :smile:
```

Common shortcodes include `:smile:`, `:heart:`, `:wave:`, `:tada:`, `:rocket:`, and many more.

#### Figure with Caption

Images with title attributes are automatically rendered as semantic HTML figures:

```markdown
![Alt text](image.png "This becomes the caption")
```

Renders as:
```html
<figure>
  <img src="image.png" alt="Alt text" title="This becomes the caption" />
  <figcaption>This becomes the caption</figcaption>
</figure>
```

### Raw HTML

Raw HTML is allowed in Markdown and will be rendered as-is. This enables you to use custom HTML elements when needed:

```markdown
This is **markdown** with <span class="custom">HTML elements</span>.

<div class="callout">
  Custom HTML blocks are also supported.
</div>
```

### Code Blocks

Fenced code blocks with syntax highlighting support:

````markdown
```rust
fn main() {
    println!("Hello, world!");
}
```
````

The language identifier is included in the HTML output as `lang="rust"` on the `<pre>` tag for syntax highlighting by your theme's JavaScript.
