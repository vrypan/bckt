![social preview card](assets/bckt-social-preview-card.png)

# bckt

`bckt` (pronounced "bucket") is an opinionated static site generator designed for personal blogs. It favors readable defaults, fast rebuilds, and simple customization so you can focus on writing instead of wiring.

## Download Pre-built Binaries

1. Go to the [Releases](https://github.com/vrypan/microblog-ssg/releases).
2. Pick the archive that matches your platform (macOS, Linux, or Windows) and download it.
3. Unpack the archive and place the `bckt` (or `bckt.exe`) binary somewhere on your `PATH`.
4. Run `bckt --help` to confirm it works.

If you prefer building the binary yourself, `cargo install --path .` inside this repository works too.

## Quick Start

```bash
# 1. Create a new site (run inside an empty directory)
bckt init

# 2. Render the site once to produce html/
bckt render

# 3. Start a local preview server with live reload
bckt dev --verbose --changed
```

After running these commands you will have:

- `posts/` – starter content you can edit or replace.
- `templates/` & `skel/` – HTML templates and static assets copied from the default theme (`bckt3`).
- `html/` – the generated site.
- `bckt.yaml` – configuration file with sensible defaults.

Publish by serving the `html/` directory with any static host (GitHub Pages, Netlify, S3, etc.).

## Customize the Theme

The default theme lives in [`themes/bckt3/`](themes/bckt3). Its README explains the file layout, how `templates/` and `skel/` work together, and how to adapt them into your own theme. Start there if you want to tweak typography, navigation, or ship a completely custom look.

## Command Overview

| Command | Purpose |
|---------|---------|
| `bckt init` | Create the starter structure (`html/`, `posts/`, `templates/`, `skel/`, `bckt.yaml`). Idempotent and safe to rerun. |
| `bckt render [--posts] [--static] [--changed|--force] [-v]` | Render posts, copy static assets, and write output into `html/`. Flags let you limit the rebuild or force a clean sweep. |
| `bckt clean` | Remove `html/` and the `.bckt/` build cache, then recreate an empty `html/` directory. Also available as `bckt clear`. |
| `bckt dev [--host] [--port] [--changed] [--verbose]` | Run a local preview server with live reload. |
| `bckt themes list` / `bckt themes use <name>` | Inspect bundled themes and copy one into your project, updating `bckt.yaml`. |

Run `bckt <command> --help` for full flag descriptions.

## Configuration

`bckt.yaml` drives site-wide settings. Every field is optional and defaults to:

```
base_url: "https://example.com"
homepage_posts: 5
date_format: "[year]-[month]-[day]"
paginate_tags: true
default_timezone: "+00:00"
```

`base_url` must be an absolute `http`/`https` URL, `homepage_posts` controls the number of entries on the landing page, `paginate_tags` toggles cursor-based tag archives, `default_timezone` is used when posts omit a timezone, and `date_format` accepts either a custom [`time` format description`](https://docs.rs/time/latest/time/format_description/) or `RFC3339`. Templates receive the configuration as `config`, and `{{ now() }}` (or `{{ now('RFC3339') }}`) renders the current timestamp.

## Posts

Store posts under `posts/` — each directory with exactly one `.md` or `.html` file becomes a post and the rest of the files in that directory are copied as attachments. Markdown is rendered with GitHub-flavored extensions and the first paragraph becomes the excerpt (trimmed to about 280 characters). Every post starts with YAML front matter:

```
---
title: "Optional title"
date: "2025-03-12T09:30:00Z"
slug: "custom-slug"
tags:
  - example
abstract: "Short teaser"
attached:
  - files/data.csv
images:
  - cover.jpg
video_url: "https://example.com/video.mp4"
---
Body goes here...
```

`slug` falls back to the directory name (kebab-cased) when omitted. Dates may use RFC 3339 or a naive `YYYY-MM-DD HH:MM:SS` timestamp (interpreted using `default_timezone`). The permalink format is `/yyyy/mm/dd/slug/`. The homepage lists the most recent `homepage_posts`, tag pages live under `/tags/<tag>/`, and monthly/yearly archives are written automatically.

## Pages

Drop standalone HTML files in `pages/` to render them as Minijinja templates. The directory structure is mirrored in `html/`, so `pages/404.html` becomes `html/404.html` and `pages/about/index.html` becomes `html/about/index.html`. Pages share the same globals as posts (`config`, `feed_url`, `now()`, etc.).

## Development

To work on bckt itself:

```
cargo fmt
cargo clippy -- -D warnings
cargo test
```
