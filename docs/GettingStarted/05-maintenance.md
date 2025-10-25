# Maintaining Your Blog

This guide covers common tasks for managing your blog over time.

## Adding New Posts

Creating posts regularly is straightforward with bckt-new:

```bash
bckt-new --title "My Latest Thoughts" --tags "tech,opinion"
```

Or use the interactive mode:

```bash
bckt-new
```

After creating your post:

1. Edit the content in your favorite editor
2. Preview with `bckt dev`
3. Build and deploy when ready

## Updating Existing Posts

To update a post:

1. Navigate to the post directory:
```bash
cd posts/2024/01/my-post
```

2. Edit `post.md` or `post.html`

3. Rebuild:
```bash
bckt render
```

The incremental build will only regenerate the changed post and related pages (archives, tags, RSS).

## Managing Tags

### Viewing All Tags

Your blog automatically generates tag pages. Access them at:
```
https://yourblog.com/tag/tagname/
```

### Adding Tags to Posts

Edit the frontmatter:

```yaml
tags:
  - technology
  - tutorial
  - beginners
```

Or as a comma-separated string:

```yaml
tags: technology, tutorial, beginners
```

### Renaming Tags

To rename a tag across all posts:

1. Use find and replace in your editor across `posts/` directory
2. Search for `- old-tag-name` and replace with `- new-tag-name`
3. Rebuild: `bckt render --force`

### Tag Pagination

Enable pagination for tag pages in `bckt.yaml`:

```yaml
paginate_tags: true
homepage_posts: 10  # Also controls posts per tag page
```

## Working with Themes

### Switching Themes

List installed themes:

```bash
bckt themes list
```

Switch to a different theme:

```bash
bckt themes use another-theme
```

Rebuild after switching:

```bash
bckt render --force
```

### Installing New Themes

Download a theme from GitHub:

```bash
bckt themes download my-theme \
  --github owner/repo \
  --subdir themes/my-theme \
  --tag v1.0.0
```

Or from a direct URL:

```bash
bckt themes download my-theme --url https://example.com/theme.zip
```

Then activate it:

```bash
bckt themes use my-theme
```

### Customizing Your Theme

You can customize your active theme by editing files in `templates/` and `skel/` directories:

- **templates/**: HTML templates (uses MiniJinja syntax)
- **skel/**: Static assets (CSS, JavaScript, images)

After making changes:

```bash
bckt render --force
```

To preserve your customizations, consider:
- Creating a custom theme in `themes/my-custom-theme/`
- Version controlling your changes
- Documenting your customizations

## Managing Static Assets

### Adding Images and Files

Static files in `skel/` are copied directly to `html/`:

```
skel/
├── favicon.ico
├── style.css
├── images/
│   ├── logo.png
│   └── banner.jpg
└── downloads/
    └── resume.pdf
```

Access them in templates and posts using paths relative to your site root:

```markdown
![Logo](/images/logo.png)
[Download Resume](/downloads/resume.pdf)
```

### Updating CSS

Edit your theme's CSS in `skel/`:

```bash
# For bckt3 theme
vim skel/style.css
```

Then rebuild static assets:

```bash
bckt render --static
```

## Site Configuration Updates

### Changing Site Title or Description

Edit `bckt.yaml`:

```yaml
title: "New Blog Title"
description: "Updated description"
```

Rebuild completely:

```bash
bckt render --force
```

### Updating Base URL

If you change domains:

```yaml
base_url: "https://newdomain.com"
```

Then:

```bash
bckt clean
bckt render
```

This ensures all links and URLs are updated.

### Adjusting Homepage Posts

To show more or fewer posts on your homepage:

```yaml
homepage_posts: 15  # Default is 5
```

Rebuild:

```bash
bckt render --force
```

## Search Index Maintenance

If you've enabled search, the index is automatically regenerated during builds. To force a rebuild:

```bash
bckt render --force
```

The search index is written to the path specified in `bckt.yaml`:

```yaml
search:
  asset_path: assets/search/search-index.json
```

## Backup and Version Control

### Using Git

Initialize a repository if you haven't:

```bash
git init
```

Create `.gitignore`:

```
html/
.bckt/
```

Commit your source files:

```bash
git add .
git commit -m "Initial commit"
```

Push to GitHub:

```bash
git remote add origin https://github.com/yourusername/your-blog.git
git push -u origin main
```

### What to Back Up

Always version control:
- `posts/` - Your content
- `bckt.yaml` - Configuration
- `pages/` - Custom pages
- Custom templates and styles (if you've modified them)

No need to version control:
- `html/` - Generated output
- `.bckt/` - Build cache
- `themes/` - Can be re-downloaded

## Performance Optimization

### Incremental Builds

Use incremental builds during development:

```bash
bckt render  # Only rebuilds changed files
```

### Full Rebuilds

Use full rebuilds when:
- Switching themes
- Changing configuration
- Strange rendering issues appear

```bash
bckt render --force
```

### Clean Builds

If you encounter build issues:

```bash
bckt clean
bckt render
```

## Organizing Old Posts

### Creating Archive Sections

You can organize old posts by creating date-based directories:

```
posts/
├── 2024/
│   ├── 01/
│   └── 02/
├── 2023/
│   ├── 12/
│   └── 11/
└── archive/
    └── 2022/
```

bckt will still find and index all posts regardless of directory depth.

### Hiding Posts Without Deleting

Create a drafts or archive folder with `.bcktignore`:

```bash
mkdir posts/archive
touch posts/archive/.bcktignore
mv posts/2020 posts/archive/
```

Posts in `archive/` won't be rendered but are preserved.

## Monitoring Your Blog

### RSS Feed

Your blog automatically generates an RSS feed at `/rss.xml`. Share this with readers:

```
https://yourblog.com/rss.xml
```

### Analytics

Add analytics by editing your theme's `base.html` template:

```html
<!-- In templates/base.html, before </head> -->
<script async src="https://analytics.example.com/script.js"></script>
```

## Troubleshooting Common Issues

### Posts Not Appearing

Check:
- Date is in correct ISO 8601 format: `2024-01-15T12:00:00Z`
- No `.bcktignore` file in parent directory
- Post directory follows structure: `posts/YYYY/MM/slug/post.md`

### Images Not Loading

Check:
- Image path is correct in Markdown: `images/photo.jpg`
- Image is listed in `attached:` frontmatter
- Image file exists in post directory

### Theme Changes Not Showing

```bash
bckt render --force  # Force full rebuild
```

### Build Errors

View verbose output:

```bash
bckt render --verbose
```

Or clean and rebuild:

```bash
bckt clean
bckt render --verbose
```

## Getting Help

If you encounter issues:

1. Check the [bckt documentation](https://github.com/vrypan/bckt/tree/main/docs)
2. Review the [README](https://github.com/vrypan/bckt/blob/main/README.md)
3. Open an issue on [GitHub](https://github.com/vrypan/bckt/issues)

## Next Steps

You now have everything you need to maintain a successful blog with bckt!

For advanced topics, check out the detailed documentation:
- [Posts Guide](../posts.md) - Deep dive into post features
- [Templates Guide](../templates.md) - Customizing templates
- [Search Configuration](../search.md) - Advanced search setup
- [Theme Development](../theme-hosting.md) - Creating your own themes
