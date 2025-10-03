# bucket3 – Project Goals

`bucket3` is a static site generator written in Rust.

The generated site will have the following characteristics:

## Configuration
- A root-level file `bucket3.yaml` defines site-wide settings (title, base URL, number of posts on the homepage, etc.).
- All values in `bucket3.yaml` are injected into the templating engine.

## Source Layout
- All posts live under `posts/`, in an arbitrary directory structure.
- Each post resides in its own directory and must contain **one main content file** (`.md` or `.html`).
- Posts may include other assets (images, video, attachments) in the same folder.
- Each content file begins with YAML frontmatter (required), supporting fields like:
  - `title` (optional, articles without titles are allowed)
  - `slug`
  - `date`
  - `tags`
  - `abstract`
  - `attached` (list of files to copy alongside the post)
  all frontmatter fields are injected into the templating engine.

## Output Layout
- Generated HTML is placed under `html/`.
- Posts are written to `/yyyy/mm/dd/slug/index.html`.
- Files listed in `attached` are copied into the corresponding output folder.
- Each tag generates a page: `/tags/<tag>/index.html`.
- The homepage lists the body of the last `N` posts (`N` from `bucket3.yaml`).
- Additional outputs: `/rss.xml` and `/sitemap.xml`.
- Pagination uses cursor-based, immutable pagination 

## Commands

### `bucket3 init`
Creates initial structure if missing:
html/       → generated site
posts/      → source posts
templates/  → HTML templates
skel/       → static assets (JS, CSS, etc.)
bucket3.yaml

### `bucket3 clear`
Deletes everything under `html/` (previously generated pages).

### `bucket3 render [--posts] [--static] [--rss] [--sitemap]`
Generates the static site. With no flags, all outputs are regenerated.

- File status under posts/ is stored in a kv-store, that allows the engine to identify new/modified files.
- the kv-store is also used to create pagination, without the need to update every page.
- There is a dependency graph: index pages, tag pages, rss and sitemap depend on posts. If one post is added or updated that affects a specific index page or tag page, then these pages should be updated too.
