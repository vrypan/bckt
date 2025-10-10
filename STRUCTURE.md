# Project Structure

This document summarises the core modules under `src/` and the responsibilities of the files that belong to each module.

## Root (`src/`)
- `main.rs`: binary entry point that wires CLI parsing with the command dispatcher and handles non-zero exit codes on failure.
- `cli.rs`: defines the `bckt` command-line interface (arguments, subcommands, and shared option structs) using `clap`.
- `markdown.rs`: wraps `comrak` to render Markdown into HTML while extracting short excerpts for listings.
- `search.rs`: builds the JSON search index from rendered posts, including facet aggregation and digest computation.
- `theme.rs`: downloads and extracts theme archives (zip or GitHub) and provides source descriptors used by commands.
- `utils.rs`: helpers that are broadly useful across the crate (currently absolute URL resolution).

## Module: `config` (`src/config/`)
- `mod.rs`: module entry point that re-exports public configuration types and functions.
- `model.rs`: defines the main `Config` struct with load, save, and validation methods; coordinates validation of all configuration fields.
- `search.rs`: search configuration models (`SearchConfig`, `SearchLanguageConfig`), default language settings, stopwords, and search config validation.
- `timezone.rs`: parses timezone strings (UTC offsets like `+00:00` or `UTC`/`Z` keywords) into `UtcOffset` values.
- `date_format.rs`: validates date format strings using the `time` crate's format description parser.
- `project.rs`: discovers the project root by walking up the directory tree to find `bckt.yaml`.

## Module: `commands` (`src/commands/`)
- `mod.rs`: dispatches parsed CLI commands to the appropriate implementation module.
- `clean.rs`: implements the `bckt clean` command (removes `html/` output and cache directories, recreates scaffolding).
- `config.rs`: implements the `bckt config` command (queries configuration values from `bckt.yaml` or returns the project root path).
- `dev.rs`: implements the file-watching development server, including initial render, live-reload polling endpoint, and static file serving.
- `init.rs`: initialises a new workspace (creates directories, downloads a theme when required, seeds config/templates/assets/sample post).
- `render.rs`: turns CLI render flags into a `RenderPlan` and invokes the renderer.
- `themes.rs`: implements `bckt themes` subcommands for listing, switching, and downloading themes (including GitHub parsing).

## Module: `content` (`src/content/`)
- `mod.rs`: discovers posts, parses front matter, renders bodies via Markdown, normalises metadata (language, tags, attachments), and exposes the `Post` model consumed by downstream pipelines.

## Module: `render` (`src/render/`)
- `mod.rs`: high-level orchestrator that evaluates a `RenderPlan`, coordinates cache state, and invokes the specialised submodules listed below.
- `assets.rs`: computes hashes for static and theme assets, copies assets into `html/`, and validates theme asset paths.
- `cache.rs`: utility helpers for opening the sled cache database and reading/writing typed entries.
- `feeds.rs`: renders RSS feeds (site-wide and tag-specific) and generates the XML sitemap using post data.
- `listing.rs`: handles homepage pagination, tag index pages, and archive generation, including cache pruning and output path helpers.
- `pages.rs`: renders static HTML files found under `pages/` through the templating environment.
- `posts.rs`: renders individual posts, manages post digests, asset copying, templating context construction, and value normalisation helpers shared with listings/feeds.
- `templates.rs`: loads templates from disk into the Minijinja environment and enriches error reporting for template render failures.
- `tests.rs`: integration-style tests that exercise the rendering pipeline end-to-end using temporary workspaces.
- `utils.rs`: shared helpers for the renderer (logging, cache digests, filesystem cleanup, date formatting, XML utilities, etc.).

## Module: `template` (`src/template/`)
- `mod.rs`: builds the Minijinja environment, injects globals/functions, and registers filters.
- `filters.rs`: implements custom Jinja filters (currently `format_date`) with strftime-style format support and caching.

## Module: `extras` (`src/extras/`)
- `bckt_fc.rs`: standalone helper binary that fetches Farcaster casts and scaffolds posts (handles API calls, attachment downloads, and mention resolution).
- `bckt_new.rs`: standalone helper binary that interactively scaffolds new posts (prompts for metadata, validates input, and writes front matter files).

