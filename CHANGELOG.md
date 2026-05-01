# Changelog

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
