# bucket3rs

`bucket3rs` is a static site generator that builds a microblog-friendly HTML tree.

## Development

```
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## Usage

```
bucket3 init
```

The `init` command creates the starter structure: `html/`, `posts/`, `templates/`, `skel/`, and a `bucket3.yaml` configuration file. The command is idempotent and prints `Initialized` when the workspace is ready.

### Configuration

`bucket3.yaml` drives site-wide settings. All fields are optional; missing values fall back to:

```
base_url: "https://example.com"
homepage_posts: 5
date_format: "[year]-[month]-[day]"
```

`base_url` must be an absolute `http` or `https` URL, `homepage_posts` must be positive, and `date_format` accepts either a custom [`time` format description`](https://docs.rs/time/latest/time/format_description/) or the keyword `RFC3339`. The configuration is injected into templates as `config`, and templates can call `{{ now() }}` (or `{{ now('RFC3339') }}`) to render the current timestamp.

### Posts

Store posts under `posts/` in any directory layout. Each directory that contains exactly one Markdown or HTML file (with a `.md` or `.html` extension) is considered a post; all other files in that directory are treated as assets. Every post file must start with YAML front matter:

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
Body goes here…
```

`slug` falls back to the directory name (kebab-cased) when omitted. Dates must use RFC 3339, and the permalink for a post is `/yyyy/mm/dd/slug/`. The `attached` and `images` lists stay relative to the post directory so later build steps can copy them alongside the rendered HTML.
