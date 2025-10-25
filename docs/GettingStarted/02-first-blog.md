# Setting Up A New Blog

This guide walks you through creating a new bckt blog from scratch.

## Initialize Your Blog

Create a new directory for your blog and initialize it:

```bash
mkdir my-blog
cd my-blog
bckt init
```

This command creates the following structure:

```
my-blog/
├── bckt.yaml          # Configuration file
├── posts/             # Your blog posts go here
├── templates/         # Theme templates
├── skel/              # Static assets (CSS, JS, images)
├── pages/             # Standalone pages (About, Contact, etc.)
└── themes/            # Downloaded themes
```

The `html/` directory will be created when you first build your site.

## Using a Custom Theme

By default, `bckt init` uses the `bckt3` theme. To start with a different theme:

```bash
bckt init --theme-github owner/repo --theme-subdir path/to/theme
```

For example, this will re-install the default theme:

```bash
bckt init --theme-github vrypan/bckt --theme-subdir themes/bckt3
```

## Configure Your Blog

Edit the `bckt.yaml` file to set up your site. Here's a minimal configuration:

```yaml
title: "My Awesome Blog"
description: "Thoughts on technology, life, and everything in between"
base_url: "https://myblog.com"
homepage_posts: 10
date_format: "[year]-[month]-[day]"
```

### Key Configuration Options

- **base_url**: Your site's URL (required - must start with http:// or https://)
- **title**: Your blog's name
- **description**: Meta description for search engines and social shares
- **homepage_posts**: Number of posts to show on the homepage (default: 5)
- **date_format**: How dates are displayed (uses Rust time crate format)
### Optional Settings

```yaml
open_graph_image: "/og-image.png"     # Default social sharing image
paginate_tags: true                   # Enable pagination on tag pages
default_timezone: "+00:00"            # Timezone for posts (UTC offset)
```

### Search Configuration

The bckt3 theme includes client-side search. Configure it like this:

```yaml
search:
  asset_path: assets/search/search-index.json
  default_language: en
  languages:
    - id: en
      name: English
```

## Preview Your Blog

Start the development server to see your blog:

```bash
bckt dev
```

This will:
- Build your site
- Start a local server at http://127.0.0.1:4000
- Watch for changes and automatically rebuild

Open your browser and visit http://127.0.0.1:4000

You should see an empty blog with the default theme!

## Understanding the Directory Structure

### posts/

Your blog content lives here, organized in folders. I like using folders named after year/month, but you can pick your own way of organizing posts. For example, you can have `/long/` and `/short/` and `/essays/` forders and organize your posts accordingly -bckt doesn't care.

```
posts/
└── 2024/
    └── 01/
        └── my-first-post/
            ├── post.md       # The post content
            └── images/       # Post-specific images
```

### templates/

MiniJinja HTML templates that define your site's structure and design. These come from your theme but can be customized.

### skel/

Static assets like CSS, JavaScript, images, and fonts. These are copied directly to your `html/` output directory.

### pages/

Standalone pages like About, Contact, or other custom pages that aren't blog posts.

### themes/

Downloaded themes are stored here. You can have multiple themes installed and switch between them.

## Next Steps

Your blog is set up! Now it's time to create your first post.

Continue to: [Creating Your First Post](03-creating-posts.md)
