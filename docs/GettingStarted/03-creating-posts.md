# Creating Your First Post

This guide shows you how to create and write blog posts with bckt.

## Using bckt-new (Recommended)

The easiest way to create a new post is with the `bckt-new` tool:

```bash
bckt-new
```

This will prompt you for:

- **Title**: The post title
- **Slug**: URL-friendly identifier (auto-generated from title if left empty)
- **Tags**: Comma-separated tags (optional)
- **Abstract**: Short summary for listings and social shares (optional)
- **Type**: Post type like "article", "video", "photo" (optional)
- **Language**: Language code like "en" or "es" (optional)

Example session:

```
Post title: My First Blog Post
Post slug (leave empty to auto-generate):
Tags (comma-separated, leave empty to skip): tutorial, beginner
Abstract (leave empty to skip): A guide to getting started with blogging
Post type (leave empty to skip): article
Post language (leave empty to skip): en
Date (YYYY-MM-DD or RFC3339, leave empty for now):

Created: posts/2024/01/my-first-blog-post/post.md
```

### Non-Interactive Mode

If you prefer to skip the prompts:

```bash
bckt-new --title "My Post" --slug "my-post" --tags "tag1,tag2" --no-prompt
```

## Post Structure

bckt-new creates a directory structure like bellow. 

> [!WARNING]
  Every post must be in its own folder.

The post folder can contain additional assets (for example image files) referenced by the post.
```
posts/
└── 2024/01/
    └── my-first-blog-post/
        ├── post.md       # Your content goes here
        └── image.png     
        └── demo-script-mentioned-in-post.sh     
```

## Writing Your Post

Open the `post.md` file in your favorite editor. You'll see frontmatter at the top:

```yaml
---
title: "My First Blog Post"
slug: "my-first-blog-post"
date: "2024-01-15T12:00:00Z"
tags:
  - tutorial
  - beginner
abstract: "A guide to getting started with blogging"
type: "article"
language: "en"
---
```

Below the frontmatter (after the `---`), write your content in Markdown.

## Markdown Example

Here's a complete example post:

```markdown
---
title: "My First Blog Post"
slug: "my-first-blog-post"
date: "2024-01-15T12:00:00Z"
tags:
  - tutorial
  - beginner
abstract: "A guide to getting started with blogging"
---

# Welcome to My Blog

This is my first post using bckt!

## What I'll Write About

I plan to cover:

- Technology and programming
- Personal projects
- Lessons learned along the way

## Code Examples

Here's a simple code snippet:

\`\`\`python
def hello_world():
    print("Hello, bckt!")

hello_world()
\`\`\`

## Lists and Tables

Shopping list:
- Milk
- Eggs
- Bread

| Feature | Status |
|---------|--------|
| Fast | ✓ |
| Easy | ✓ |

## Images

![My awesome image](images/photo.jpg)

> [!NOTE]
> bckt supports GitHub-style alerts!

Looking forward to sharing more soon.
```

## Frontmatter Fields

### Required

- **date**: ISO 8601 timestamp (e.g., `2024-01-15T12:00:00Z`)

### Recommended

- **title**: Post title (defaults to slug if missing)
- **slug**: URL identifier (defaults to directory name)
- **tags**: Array or comma-separated string
- **abstract**: Summary for listings and social media

### Optional

- **type**: Custom post type for template customization
- **language**: Language code for multilingual sites
- **image**: Featured image path (e.g., `images/cover.jpg`)
- **attached**: Array of files to copy with the post

## Adding Images

1. Add your image files in the post folder
2. Reference them in your Markdown:

```markdown
![Alt text](my-photo.jpg)
```

To include the image in your published site, add it to frontmatter:

```yaml
attached:
  - my-photo.jpg
```

Or set it as the featured image:

```yaml
image: my-photo.jpg
attached:
  - my-photo.jpg
```

## Markdown Features

bckt uses GitHub Flavored Markdown with extensions:

- **Tables**: Pipe-separated tables
- **Strikethrough**: `~~text~~`
- **Task lists**: `- [ ]` and `- [x]`
- **Footnotes**: `[^1]` and `[^1]: footnote text`
- **Alerts**: `> [!NOTE]`, `> [!WARNING]`, `> [!TIP]`
- **Code blocks**: With syntax highlighting
- **Raw HTML**: Allowed in Markdown
- **Emoji**: `:smile:` becomes 😊

## Preview Your Post

With the dev server running:

```bash
bckt dev
```

Save your changes and refresh your browser. The page will automatically rebuild!

## Creating Posts Manually

If you prefer not to use bckt-new, create the directory structure manually:

```bash
mkdir -p posts/2024/01/my-post
touch posts/2024/01/my-post/post.md
```

Then add the frontmatter and content yourself.

## Working with Drafts

To exclude posts from rendering, create a `.bcktignore` file in the directory:

```bash
mkdir posts/drafts
touch posts/drafts/.bcktignore
```

Any posts in `drafts/` (and its subdirectories) will be ignored during builds.

## Next Steps

You've created your first post! Now let's learn about building and publishing your blog.

Continue to: [Building and Publishing](04-publishing.md)
