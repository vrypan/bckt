# Client-Side Search

The `bckt` renderer produces a client-side JSON index at `/assets/search/search-index.json`. Themes can load this index in the browser to power instant, offline search experiences. This document explains how the index is generated, how to configure analyzers, and how to wire the theme-side JavaScript.

## Index generation

- Every render pass builds a search index from all published posts. The index is regenerated during incremental builds whenever post content or relevant configuration changes.
- The JSON payload contains:
  - `documents`: one entry per post with title, excerpt, permalink, language, tags, type, timestamps, and the plain-text body used for full-text search.
  - `languages`: analyzer metadata (identifier, display name, stopword list) exposed for the client UI.
  - `facets`: precalculated lists of tags, types, and publication years for building filter widgets.
- The index lives under `html/assets/search/search-index.json`. Adjust the target path with `search.asset_path` in `bckt.yaml` if you serve assets from a different prefix.

## Configuring analyzers and stopwords

Search behaviour is controlled in `bckt.yaml` under the `search` key. English (`en`) and Greek (`el`) analyzers ship by default. Additional languages are added declaratively—no Rust changes required.

```yaml
search:
  asset_path: assets/search/search-index.json
  default_language: en
  languages:
    - id: en
      name: English
      stopwords:
        - a
        - an
        - and
        - the
    - id: el
      name: Ελληνικά
      stopwords:
        - και
        - να
        - το
    - id: fr
      name: Français
      stopwords:
        - et
        - le
        - la
        - les
```

Guidelines:

- `id` should be a BCP-47 language tag (`en`, `en-GB`, `el`, etc.). ISO-639-3 codes (e.g. `eng`) are also accepted; the renderer normalises them using the active analyzers.
- `stopwords` is optional. Provide lowercase tokens; they are de-duplicated automatically.
- `default_language` must match one of the configured analyzers and is used whenever the language cannot be detected.

## Theme integration checklist

The `bckt3` theme provides a ready-to-use wiring in `themes/bckt3/templates/search.html` and `themes/bckt3/assets/js/search.js`. To port these hooks into another theme:

1. **Navigation link** – add a link to `/search/` in the primary navigation so visitors can reach the search page.
2. **Search page template** – create a template (e.g. `templates/search.html`) that:
   - extends your base layout,
   - renders a search input plus filter controls, and
   - loads the JSON index path through a `data-search-index` attribute.
3. **Client scripts** – include two scripts at the bottom of the page (deferred):
   - `assets/js/minisearch.js`: a vendored MiniSearch-compatible indexer.
   - `assets/js/search.js`: the theme controller that fetches the JSON index, builds the in-memory MiniSearch instance, and binds filters.
4. **Static assets** – place all JavaScript assets under `themes/<theme>/assets/`. The renderer copies this directory to `html/assets/` during the static-assets stage, keeping filenames stable for caching.
5. **Styling** – add CSS for the search page (`search-page`, `search-field__input`, `search-card`, etc.) so that the results match the rest of the theme.

The search controller exposes filters for language, tag, type, and publication year. Facet values come from the generated JSON. Update `themes/bckt3/assets/js/search.js` if you need bespoke behaviour (for example, additional filters or custom result rendering).

## Incremental builds

Search index updates participate in incremental renders:

- The renderer stores a hash of the search payload in the cache database. When posts change (or search-related configuration updates), the hash changes and the JSON file is rewritten.
- Running `bckt render --changed` after editing a post regenerates the search index automatically. No manual cache invalidation is required.

If you relocate the search JSON or customise analyzers, re-run a full build (`bckt render --force` or delete the `.bckt` cache directory) to repopulate the cache with the new settings.
