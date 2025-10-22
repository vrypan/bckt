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
