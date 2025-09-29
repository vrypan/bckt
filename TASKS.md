# TASKS

This file tracks the work plan for **bucket3rs**. Tasks are grouped into milestones with crisp acceptance criteria so each can land as a clean PR.

---

## Ground Rules

- **Rust edition:** 2021 (or 2024 if stable).
- **Quality bar:** `cargo fmt`, `cargo clippy -- -D warnings`, unit tests passing on CI.
- **Error handling:** `anyhow` for app-level errors; `thiserror` for library-style modules.
- **Docs:** Each PR must update README / usage examples when behavior changes.

---

## Milestone 0 — Repo Bootstrap (PR: `feat:init`)
**Goal:** Compile, run `bucket3 init`, and create the skeleton.

- [ ] Cargo setup with deps: `clap`, `anyhow`, `serde`, `serde_yaml`, `minijinja`, `comrak`, `sled`, `walkdir`, `time`.
- [ ] Binary `bucket3`.
- [ ] Command: `bucket3 init` creates `html/`, `posts/`, `templates/`, `skel/`, `bucket3.yaml` (idempotent; won’t overwrite).
- [ ] Seed minimal templates (`base.html`, `post.html`, `index.html`) and sample post.
- [ ] CI: GitHub Actions workflow (fmt, clippy, test).
**DoD:** Running `bucket3 init` on an empty repo completes without error and prints “Initialized”.

---

## Milestone 1 — Config & Template Context (PR: `feat:config`)
**Goal:** Load `bucket3.yaml` and expose to templates.

- [ ] `Config` struct + `serde_yaml` loader with defaults (`homepage_posts`, `date_format`, `base_url`).
- [ ] Validate base URL and numeric ranges; nice error messages.
- [ ] Inject `config` into template context; add `now` (current time) helper.
- [ ] Unit tests: valid/invalid config; missing file falls back to defaults.
**DoD:** `{{ config.title }}` renders in templates; invalid YAML shows actionable error.

---

## Milestone 2 — Content Model & Front-Matter (PR: `feat:content-model`)
**Goal:** Parse posts from `posts/` tree.

- [ ] Discover post directories recursively under `posts/`.
- [ ] Each post must have **one** main file: `.md` or `.html`.
- [ ] YAML front-matter schema:
  - `title` (optional), `date` (RFC 3339), `tags: []`, `abstract`, `slug` (optional), `attached: []`
  - Media hints: `images: []`, `video_url` (optional)
- [ ] Slug rules: use front-matter `slug` else directory name (kebab).
- [ ] Compute permalink: `/yyyy/mm/dd/slug/`.
- [ ] Tests: date parsing, slugify, invalid/missing main file, front-matter only.
**DoD:** A `Post` struct is produced for sample content with correct permalink.

---

## Milestone 3 — Markdown → HTML (PR: `feat:markdown-gfm`)
**Goal:** Render Markdown with GFM features.

- [ ] `comrak` wrapper with options: tables, task lists, strikethrough, autolinks, footnotes.
- [ ] Front-matter split (`---` fence) and pass body to renderer.
- [ ] Generate `excerpt` (first N chars / until first paragraph) for feed.
- [ ] Tests: GFM features, fenced code, footnotes, excerpt edge cases.
**DoD:** Given sample Markdown with GFM, `post.html` contains expected HTML.

---

## Milestone 4 — Rendering Pipeline (PR: `feat:render-pipeline`)
**Goal:** Build all posts into `html/` with templates.

- [ ] Minijinja `Environment` with template inheritance.
- [ ] Render post pages to `/yyyy/mm/dd/slug/index.html`.
- [ ] Copy `attached` assets (verify existence; error if missing).
- [ ] Copy `skel/` to `html/` (static assets) preserving structure.
- [ ] Command: `bucket3 render --posts --static` (both by default).
- [ ] Tests: post output pathing; attached files present; missing asset error surfaced.
**DoD:** `bucket3 render` produces working HTML for the sample post + assets.

---

## Milestone 5 — Homepage & Basic Pagination (PR: `feat:homepage`)
**Goal:** Chronological feed and pager.

- [ ] Sort posts by `date` desc.
- [ ] Homepage shows last `homepage_posts` bodies (no-title friendly).
- [ ] Pagination: `/page/2/` etc., with “Newer/Older” links.
- [ ] Template context includes `pagination.{prev,next,current,total}`.
- [ ] Tests: page counts at boundaries (0, 1 page, many pages).
**DoD:** Visiting `/` and `/page/2/` shows correct posts and nav.

---

## Milestone 6 — Tags & Archives (PR: `feat:tags-archives`)
**Goal:** Tag indexes + date archives.

- [ ] Tag pages at `/tags/<tag>/index.html` (alpha by tag, reverse-chron by posts).
- [ ] Optional pagination for tags (config: `paginate_tags`).
- [ ] Year/month archive pages: `/2025/` and `/2025/09/` (optional if time-limited).
- [ ] Tests: tag with 1 post, many posts; special chars in tags (normalize to path).
**DoD:** Tag and archive pages render with correct counts and links.

---

## Milestone 7 — Feeds & SEO (PR: `feat:feeds-seo`)
**Goal:** RSS + sitemap + metadata.

- [ ] Generate `/rss.xml` (last 50 posts, absolute URLs).
- [ ] Generate `/sitemap.xml` (posts, tags, homepage, pages).
- [ ] `<link rel="alternate" type="application/rss+xml">` in base template.
- [ ] Tests: well-formed XML, absolute URLs based on `base_url`.
**DoD:** Feed validates in common validators; sitemap indexes all pages.

---

## Milestone 8 — KV Index & Incremental Builds (PR: `feat:incremental`)
**Goal:** Faster rebuilds with `sled`.

- [ ] Store content hashes (front-matter + body + asset mtimes) per post.
- [ ] Skip rendering/copy when unchanged; detect template or config changes → invalidate all.
- [ ] Command: `bucket3 render --changed` and `--force`.
- [ ] Tests: changing a single post only rebuilds its outputs; changing base.html triggers full rebuild.
**DoD:** Re-running `render` on unchanged repo performs near-no work.

---

## Milestone 9 — Media UX niceties (PR: `feat:galleries`)
**Goal:** Multi-image posts & video polish.

- [ ] Gallery context in templates: `post.images` list; lightweight grid; `loading="lazy"`.
- [ ] Optional lightbox hook (just data-attributes; JS left to theme).
- [ ] Video: `<video controls preload="metadata">` for local files or external URL.
- [ ] Tests: 0/1/many images; missing images caught at build.
**DoD:** Sample post displays a clean gallery & playable video.

---

## Milestone 10 — CLI UX & Selective Builds (PR: `feat:cli-ux`)
**Goal:** Sharper developer experience.

- [ ] `bucket3 render --post <slug>` for a single post.
- [ ] `bucket3 clear` removes only `html/`.
- [ ] Verbose flag `-v` with timing; quiet mode `-q`.
- [ ] Exit codes consistent (nonzero on errors).
**DoD:** Selective build works; commands show useful progress.

---

## Milestone 11 — Docs & Examples (PR: `docs:usage`)
**Goal:** Make it easy to adopt.

- [ ] README: quick start, config fields, front-matter schema, URL structure.
- [ ] `examples/` with multiple posts (no-title, multi-image, video).
- [ ] `CONTRIBUTING.md` with dev loop, test commands.
**DoD:** A new user can install, init, add a post, render, and deploy in <10 min.

---

## Future / Stretch (not required for v1)
- [ ] **Stable pagination strategy** that minimizes regen churn (e.g., reverse-chron buckets by month, or “cursor” pages) to avoid rebuilding all pages on new posts.
- [ ] **Image pipeline**: thumb generation, responsive `srcset`.
- [ ] **Dev server**: `bucket3 serve` with file-watch and live reload.
- [ ] **Theme packs** and a theme registry.
- [ ] **Importers** (Micro.blog export, RSS/JSON feed import).
- [ ] **Search**: client-side JSON index or external search integration.
- [ ] **I18n**: per-post language, localized dates.
- [ ] **Content validation**: schema checks with helpful diagnostics.

---

## Non-Goals (for v1)
- Comments, authentication, or server-side dynamic features.
- Distributed builds or cloud pipelines.
- WYSIWYG editor.

---

## Acceptance Test Matrix (quick reference)

| Area          | Test                                  | Expectation                                  |
|---------------|----------------------------------------|----------------------------------------------|
| Init          | Run `bucket3 init` twice               | Second run no-ops; no overwrites              |
| Config        | Missing `bucket3.yaml`                 | Defaults applied; warning, not crash          |
| Front-matter  | Invalid date format                    | Clear error with file path + hint             |
| Markdown      | GFM table/task list/footnote           | Correct HTML output                           |
| Paths         | Permalink `/yyyy/mm/dd/slug/`          | `index.html` exactly there                    |
| Attachments   | Missing file in `attached`             | Build fails with actionable error             |
| Homepage      | Pagination boundaries                  | Correct prev/next links                       |
| Tags          | Tag with spaces/Unicode                | Safe URL segment + page renders               |
| RSS/Sitemap   | Absolute URLs                          | Use `base_url`; XML validates                 |
| Incremental   | Edit one post                          | Only that post re-renders                     |
| CLI           | `render --post <slug>`                 | Builds only that post                         |