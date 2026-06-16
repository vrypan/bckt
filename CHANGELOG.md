# Changelog

## [0.7.1] - 2026-06-17

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
