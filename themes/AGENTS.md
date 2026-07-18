# Building bckt themes with an AI agent

This file is guidance for an AI coding agent (Claude Code, Cursor, Copilot,
Codex, etc.) asked to create or modify a theme for this [bckt](https://github.com/vrypan/bckt)
blog. Read it before touching any theme files.

## Golden rule: edit theme sources, not the root copies

A theme is a self-contained directory: `themes/<name>/`. **That is the only
place you edit.**

The project root also contains `templates/` and `skel/`, but those are
**generated copies** — `bckt themes use <name>` overwrites them from the active
theme. Editing them directly is lost on the next apply. (The root `pages/` is
*not* touched by `bckt themes use`; a theme's starter pages are seeded once by
`bckt init`, and `pages/` is treated as your content thereafter.)

After **every** change under `themes/<name>/` (templates, CSS, assets — anything),
re-apply and rebuild from the project root:

```bash
bckt themes use <name> --force   # copy theme → root templates/ and skel/, update bckt.yaml
bckt render --force              # rebuild html/   (or run `bckt dev` to live-preview)
```

The root `templates/` and `skel/` are *only* refreshed by
`bckt themes use`, so skipping it renders stale files. `--force` on `use` skips
the overwrite prompt; `--force` on `render` rebuilds every post (otherwise
unchanged posts are skipped, which hides template/CSS edits that affect all
posts). Run from the project root (where `bckt.yaml` lives), or pass
`--root <path>` *before* the subcommand.

> **Seeing `failed to open cache database`?** `bckt dev` and `bckt render` can't
> hold the render cache at the same time. This usually means the user is running
> `bckt dev` in another window to live-preview — **ask them** before working
> around it. If so, you often don't need to render at all: `dev` rebuilds on its
> own when you edit theme files (after `bckt themes use`). Otherwise ask them to
> stop `dev` so you can run `bckt render`. Don't just retry blindly in a loop.

## Theme anatomy (`themes/<name>/`)

```
templates/            # MiniJinja templates (page chrome + post layouts)
skel/                 # static files copied VERBATIM to the site root (/)
pages/                # standalone pages (.html extends base.html; other files copied as-is)
```

Static assets served at `/assets/…` (e.g. the search JS) live under
`skel/assets/` — `skel/` is mirrored to the site root, so `skel/assets/js/search.js`
is published at `/assets/js/search.js`. There is no separate top-level theme
`assets/` directory.

Only `templates/` and `skel/` are required to ship a theme; `rntz` is a small
one to read first. A theme may keep any extra build sources it wants in its own
directory (a CSS preprocessor config, source art, etc.) — those are not served;
only what lands in `skel/` is.

## Creating a new theme

There is no scaffold command — start by copying the closest existing theme:

```bash
bckt themes list                  # see what's installed
cp -r themes/rntz themes/<name>   # rntz = compact pure-CSS starting point
bckt themes use <name> --force && bckt render --force   # make it active and build
```

(`bckt themes install <path-to.zip>` can instead unpack a theme archive into
`themes/`.)

Pick the starting theme by what you need: `rntz` (minimal, classic blog),
`micro` (microblog — notes, media carousel, sidebar), `bckt3` (cards),
`modern` (editorial). Nothing is inherited between themes, so your new directory
must carry its **own** copies of the shared assets (favicons, search JS,
standalone pages) — see "Static assets & CSS" below. Missing them is the usual
cause of 404s.

## Templating

Templates render with **MiniJinja** (Jinja2-compatible) during `bckt render`.

- `base.html` is the shell (`<head>`, nav, footer). Other templates
  `{% extends "base.html" %}` and override blocks.
- Blocks defined in `base.html`: `page_title`, `page_meta`, `head`, `content`,
  `page_scripts`. Use `head` for per-page `<link>`/`<style>` (e.g. a font only
  some pages need) so other pages don't pay for it.
- Core templates: `index.html` (home feed), `post.html` (default post),
  `tag.html`, `archive_year.html`, `archive_month.html`, `search.html`,
  `rss.xml`. Partials live in `templates/partials/`.
- **Post types**: a post's `type` selects its template/partial. This theme has
  `default`, and `note` variants (`post.html` / `post-note.html` 
  and the matching `partials/summary-*.html`). When you add
  or change a type, wire it into **every** listing that renders post bodies —
  `index.html`, `tag.html`, *and* the archive templates — not just `index.html`.
  Miss one and that type renders blank there (a titleless note shows nothing).

### Context available to templates

- `config` — values from `bckt.yaml`, including `config.extra`.
- `base_url` — full site URL (use for canonical links, RSS, Open Graph).
- `base_path` — path component only (use for internal links and assets;
  empty for root deployments).
- `posts` — list of `PostSummary` objects (listings/RSS).
- `pagination` — `{ current, total, prev, next }` where applicable. **bckt
  paginates in reverse ("notebook" order):** higher page numbers are newer, and
  the **home page is the highest page (`current == total`)** — like the open
  page of a notebook you're still writing in. Page 1 holds the *oldest* posts.
  Consequently:
  - `pagination.next` → the **newer** page (higher number); `pagination.prev` →
    the **older** page (lower number). Label links accordingly ("newer posts"
    for `next`, "older posts" for `prev`).
  - The home page has **no `next`** (nothing is newer) but usually **has a
    `prev`** (older posts exist). So test `not pagination.next` — *not*
    `not pagination.prev` — to detect the home/newest page (e.g. to show the
    site intro only there). Using `prev` to mean "first page" is the
    conventional-pagination assumption and is wrong here.
  - See <https://blog.vrypan.net/2025/10/13/blog-pagination-notebook-way/>.
- `tag`, `year`, `month` — on tag/archive views.
- `post` — a `PostSummary`/`PostTemplate` exposing `title`, `slug`,
  `permalink`, `date`, `date_iso`, `tags`, `body`, `excerpt`, `attachments`
  (a map of path → `{ size, mime_type }`), plus **all custom frontmatter fields**
  flattened in (this theme uses `image`, `images`, `videos`, `abstract`,
  `weather`, `location`, `castid`, …).

### Custom filter

- `format_date` — formats an RFC3339 timestamp with strftime tokens. Always
  feed it `post.date_iso`, not `post.date`:

  ```jinja
  {{ post.date_iso | format_date("%a, %d %B %Y %H:%M") }}
  ```

## Static assets & CSS

- Files in `skel/` are mirrored to the output root. Reference them
  root-relative: `<link rel="stylesheet" href="/style.css">`, `/img/...`, etc.
- **CSS.** `bckt` serves `skel/style.css` as-is at `/style.css` and does **not**
  build CSS. The simplest theme writes plain CSS straight into `skel/style.css`
  with no build step — just edit and re-apply (`rntz`, `modern`, `bckt3` are
  examples; CSS custom properties in `:root` for palette/fonts keep a theme easy
  to retune). How you author it is your call: if you prefer a preprocessor or a
  utility framework, keep its sources in the theme directory and compile them to
  `skel/style.css` yourself before `bckt themes use` — bckt only ever sees the
  output. Editing only templates (not styles) needs no rebuild of the CSS.

- **Shared assets must be included in each theme.** Favicons, search JS
  (`skel/assets/js/`), and standalone pages (`pages/`) are *not* inherited from
  other themes or the project root — each theme directory must contain its own
  copies. When creating a new theme, copy these from an existing theme:

  ```bash
  cp -r themes/existing/skel/favicon* themes/new/skel/
  cp -r themes/existing/skel/js themes/new/skel/
  cp -r themes/existing/skel/img themes/new/skel/
  cp -r themes/existing/skel/assets themes/new/skel/
  cp -r themes/existing/pages/ themes/new/pages/
  ```

  Templates like `search.html` reference `/assets/js/search.js` and standalone
  pages under `/search/` — omitting these will cause 404s.

## Working efficiently

- Keep the rendered output identical unless asked to change visuals.
- Load icon fonts / extra stylesheets only on pages that use them (via the
  `head` block, ideally guarded by an `{% if %}`), not globally in `base.html`.
  Avoid `@import` inside `<style>` — it serializes downloads; use `<link>`.
- Prefer inlining a one-off SVG icon over pulling in a whole icon-font library.

## Authoritative reference

The bckt docs are the source of truth and go deeper than this file:
<https://github.com/vrypan/bckt/tree/main/docs>
