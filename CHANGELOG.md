# Changelog

## [Unreleased]

### Changed

- **`bckt-new` names the content file after the slug**: new posts are now
  scaffolded as `posts/<YYYY>/<YYMMDD-slug>/<slug>.md` (previously the Markdown
  file repeated the dated directory name, `<YYMMDD-slug>.md`). The post
  directory is unchanged. Existing posts are unaffected — any `.md`/`.html`
  filename is still discovered.

## [0.7.5]

This release is mostly performance work on the render pipeline plus a few
correctness fixes. Several changes alter cache-key/digest formulas, so the
first render after upgrading re-renders everything once and then settles into
fast incremental rebuilds.

### Added

- **Duplicate permalinks are now a hard error**: two posts resolving to the
  same `/YYYY/MM/DD/slug/` (via a front-matter `slug:` override or a copied
  post directory) previously let one silently overwrite the other, so a post
  vanished from the site with no diagnostic. The build now fails, naming the
  colliding permalink and both source directories, so the collision gets fixed
  rather than hidden.

### Fixed

- **Errors print their full cause on exit**: a wrapped failure — notably
  `bckt dev`'s "initial render before dev server failed" — now shows the
  underlying reason (e.g. which duplicate permalink) instead of only the
  outermost message.
- **Panic on malformed timezone offsets**: a front-matter date whose trailing
  offset held a multibyte character (e.g. `+1½0`) panicked with a char-boundary
  error and crashed the whole build; it now reports the normal "invalid offset"
  error naming the file.
- **Listing pages no longer re-render nondeterministically**: post attachment
  metadata was serialized from a `HashMap` in per-process-random order, so tag
  and archive pages re-rendered a shifting random subset on every incremental
  build (churning mtimes). Attachment ordering is now stable.

### Performance

- **Rendered markdown is cached**: unchanged posts skip comrak entirely on
  incremental rebuilds, keyed by a content hash, and each content file is read
  once per render instead of twice. This is the largest win for big blogs and
  the `bckt dev` loop.
- **The search index is rewritten only when it changes**: its digest previously
  included a per-render timestamp, so the (usually largest) output file was
  reserialized and rewritten on every render, defeating mtime-based deploys.
- **Feeds and the sitemap are rewritten only when their content changes**, and
  stale `rss-<tag>.xml` files are removed when a tag leaves `rss_tags`.
- **Each post's summary is built once per render** and shared across the
  homepage, tag, archive, and feed renderers instead of 4–6 times — each build
  previously re-ran `fs::metadata` on every attachment and rescanned each body.
- **`date_format` parsing is cached** instead of re-parsed for every formatted
  date.
- **Static-asset (`skel/`) change detection hashes metadata** (path + length +
  mtime) instead of reading every file's bytes on every render.
- **The dev-server watcher is debounced**: a single editor save (which emits
  several filesystem events) now triggers one rebuild instead of two
  back-to-back.

## [0.7.4]

### Added

- **`archive_years` template global**: every template (index, posts, tag/archive
  pages, and standalone `pages/`) now receives an `archive_years` list sorted
  newest-first. Each item exposes `year` (int) and `count` (int, number of posts
  that year), making it straightforward to build a year navigator in shared
  sidebar chrome without extra context plumbing.

### Fixed

- **Homepage and paginated pages now re-render when a post's body changes**:
  incremental rebuilds (`bckt render`) previously compared only post identifiers
  (date + slug) when deciding whether to regenerate index pages. Editing a
  post's body left the homepage showing stale content until `--force` was used.
  The cache now hashes the full rendered summaries, consistent with how tag and
  year/month archive pages already worked.
- **`bckt-new` no longer tags new posts with `en`**: when `--tags` was omitted
  and the prompt was accepted without input, new posts were silently written with
  `tags: en` (a copy-paste leftover from the language-default logic). The default
  is now empty — posts have no tags unless the user supplies them.

## [0.7.3]

### Changed

- **`BCKT_THEME_PATH` renamed to `BCKT_SHARE_PATH`**: it now points to a bckt
  data root containing `themes/` and `demo/` subdirectories, rather than a
  themes directory directly. Release archives and the Homebrew formula now
  preserve this `share/bckt/{themes,demo}/` layout.
- Theme and demo lookup now derives a single data root from the executable's
  on-disk layout (Homebrew keg vs. tarball/zip) instead of probing multiple
  candidate paths, so no non-existent path is ever searched.

## [0.7.2]

### Added

- **`bckt init --demo <name>`**: populate a new project with sample posts and
  pages. Two demo sets ship with the release — `microblog` (short notes, good
  for `micro`/`microx`) and `articles` (long-form posts, good for `modern`,
  `bckt3`, `plain`, `rntz`). Demo content is resolved from `demo/<name>/`
  on the theme search path.
- **`themes/AGENTS.md`**: guidance file for AI coding agents building or
  modifying bckt themes. Reference it in your prompt with the GitHub URL.

### Changed

- **`BCKT_THEME_PATH` renamed to `BCKT_SHARE_PATH`**: it now points to a bckt
  data root containing `themes/` and `demo/` subdirectories, rather than a
  themes directory directly. Release archives and the Homebrew formula now
  preserve this `share/bckt/{themes,demo}/` layout.
- Theme and demo lookup now derives a single data root from the executable's
  on-disk layout (Homebrew keg vs. tarball/zip) instead of probing multiple
  candidate paths, so no non-existent path is ever searched.
- Release workflow now runs manually only (removed automatic push-tag and
  pull-request triggers).
- Themes: replaced hardcoded `px` root font-size with `rem` in `micro`,
  `microx`, `modern`, and `plain` so layouts scale with the user's browser
  font preference.
- `microx`: improved search panel styling for dark mode — `appearance: none`
  on form controls, elevated surface background, focus outline, card-style
  panel with padding, and a text-link style for the "More options" toggle.

## [0.7.1]

### Changed

- `bckt themes install` now resolves bare theme names (e.g. `bckt themes
  install microx`) via the theme search path (`BCKT_THEME_PATH`, executable
  directory, `<prefix>/share/bckt`), in addition to explicit `.zip` and
  directory paths.

## [0.7.0] - 2026-06-16

### Removed

- **`bckt-fc` binary**: the Farcaster companion command was removed. Theme
  rendering of `type: farcaster` posts is unchanged.
- **Remote theme download**: `bckt themes download` and the `--theme-url` /
  `--theme-github` / `--theme-tag` / `--theme-branch` / `--theme-subdir`
  flags are gone. Themes are now installed from local `.zip` archives or
  directories. All network code (and the `ureq` dependency) was removed.
- **`paginate_tags`** config key: it was never read by the renderer.
- **Automatic language detection**: language now comes from the `language:`
  front-matter field, falling back to `search.default_language`. Dropped the
  `whatlang` and `isolang` dependencies.
- **`theme.yaml`** theme metadata files (were never read).

### Added

- **`bckt themes install <path>`**: install a theme from a local `.zip` or
  directory.
- **`bckt init --theme <zip|dir|name>`**: choose the starter theme.
- **Theme search path**: `BCKT_THEME_PATH`, the executable's directory, and
  `<prefix>/share/bckt` (so Homebrew/prefix installs are auto-discovered).
- Theme archives for every bundled theme are attached to releases, and the
  themes are bundled into the release archives.

### Fixed

- **`bckt themes use` no longer destroys `pages/`**: only `templates/` and
  `skel/` are replaced; user pages are preserved.

### Changed

- Theme `assets/` were folded into `skel/assets/`; the renderer no longer
  treats theme assets specially.
- Consolidated `extract_base_path` and `split_csv` into `utils`, and the three
  cache-cleanup functions into one helper.

### Dependencies

- Removed `ureq` and `whatlang`/`isolang`.
- `tempfile` moved to dev-dependencies.

## [0.6.5] - 2026-05-01

### Fixed

- **Memory leak in dev mode**: template strings were kept alive for the entire process lifetime via `Box::leak`. Templates are now managed with `Arc<HashMap>` + minijinja's `set_loader`, so they are freed correctly on each rebuild.
- **Silent attachment errors**: missing attachment files were silently ignored. They are now reported as errors.
- **Tag RSS feed ordering**: posts in tag feeds were reversed before filtering, producing wrong results when tags were a subset of all posts. Filtering now happens before reversing.
- **Unnecessary clones in cache cleanup**: removed redundant `clone()` calls before byte-vector construction in four cache cleanup functions.

### Changed

- Duplicate language detection logic (`language_lookup`, `canonical_language`, `sanitize_language`) consolidated into `src/language.rs`.
- Duplicate `slugify` function consolidated into `src/utils.rs`; `tag_slug` in the listing renderer delegates to it.
- `att_to_absolute` rewritten with slice advancement instead of byte-index arithmetic; attribute value scanning uses `str::find` instead of a character loop.
- `build_attachments_meta` extracted to eliminate duplicate `fs::metadata` calls in post rendering.

### Dependencies

- `anyhow` 1.0.100 → 1.0.102
- `blake3` 1.8.2 → 1.8.5
- `clap` 4.5.48 → 4.5.60
- `comrak` 0.45 → 0.52
- `minijinja` 2.12.0 → 2.19.0 (added `loader` feature)
- `notify` 6.1 → 8.2
- `serde_json` 1.0.145 → 1.0.149
- `tempfile` 3.23.0 → 3.27.0
- `time` 0.3.44 → 0.3.47
- `ureq` 2.9 → 3.3
- `url` 2.5.7 → 2.5.8
- `whatlang` 0.16 → 0.18
- `zip` 0.6 → 8.6
