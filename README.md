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

```
bucket3 render [--posts] [--static] [--changed|--force] [-v|--verbose]
```

`render` processes the Markdown/HTML sources under `posts/` and writes files into `html/yyyy/mm/dd/slug/index.html`, copying any attachments listed in front matter into the same directory. Static assets under `skel/` are mirrored into `html/`. If no flags are provided, both posts and static assets are refreshed; `--posts` or `--static` limit the run to that portion of the pipeline. `--changed` reuses cached digests so only modified posts are rebuilt, while `--force` discards the cache and renders everything. Add `-v/--verbose` to see per-step progress and which posts were rendered or skipped.

```
bucket3 clean
```

`clean` removes the current `html/` directory along with the incremental build cache stored in `.bucket3/`, then recreates `html/` as an empty folder so the next render starts from a pristine state. The subcommand is also available as `bucket3 clear` for parity with the project goals document.

```
bucket3 dev [--host <host>] [--port <port>] [--changed] [--verbose]
```

`dev` starts a tiny HTTP server rooted at `html/`, recompiling the site when files in `posts/`, `templates/`, `skel/`, or `bucket3.yaml` change. Served HTML is augmented with a small polling script so connected browsers reload automatically after each rebuild. Use `--host` and `--port` to bind to a different interface, `--changed` to prefer incremental rebuilds, and `--verbose` for detailed render logs.

Each command now ships with expanded `--help` output; run `bucket3 <command> --help` to see descriptions of every flag and workflow.

### Configuration

`bucket3.yaml` drives site-wide settings. All fields are optional; missing values fall back to:

```
base_url: "https://example.com"
homepage_posts: 5
date_format: "[year]-[month]-[day]"
paginate_tags: true
default_timezone: "+00:00"
```

`base_url` must be an absolute `http` or `https` URL, `homepage_posts` must be positive, `paginate_tags` toggles cursor-based tag archives, `default_timezone` provides the offset (e.g. `+02:00`) used when posts omit a timezone, and `date_format` accepts either a custom [`time` format description`](https://docs.rs/time/latest/time/format_description/) or the keyword `RFC3339`. The configuration is injected into templates as `config`, and templates can call `{{ now() }}` (or `{{ now('RFC3339') }}`) to render the current timestamp.

### Posts

Store posts under `posts/` in any directory layout. Each directory that contains exactly one Markdown or HTML file (with a `.md` or `.html` extension) is considered a post; all other files in that directory are treated as assets. Markdown sources are rendered with GitHub-flavored options (tables, task lists, strikethrough, autolinks, and footnotes), and the first paragraph becomes the post excerpt (truncated to ~280 characters). Every post file must start with YAML front matter:

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

`slug` falls back to the directory name (kebab-cased) when omitted. Dates may use RFC 3339 or a naive `YYYY-MM-DD HH:MM:SS` timestamp (which will be interpreted with the configured `default_timezone`), and the permalink for a post is `/yyyy/mm/dd/slug/`. The `attached` and `images` lists stay relative to the post directory so later build steps can copy them alongside the rendered HTML. The homepage shows the most recent `homepage_posts` entries and writes immutable archive pages keyed by a cursor (`/page/<timestamp-slug>/`), so new posts only regenerate the head page. Tags render under `/tags/<tag>/` (with optional cursor pagination when `paginate_tags` is enabled) and yearly/monthly archives render under `/yyyy/` and `/yyyy/mm/`.
