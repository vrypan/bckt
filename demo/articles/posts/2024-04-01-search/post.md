---
title: "Built-in Search, No External Service Required"
slug: "search"
date: "2024-04-01T10:00:00Z"
tags:
  - bckt
  - features
abstract: "How bckt's client-side search works, what it indexes, and how to configure it."
language: en
attached: []
---

Most static site generators leave search as an exercise for the reader —
integrate Algolia, pay for a hosted service, or do without. bckt takes a
different approach: search is built in, generated at render time, and
requires no external dependencies.

## How it works

When you run `bckt render`, it builds a search index from all your posts
and writes it to a single JSON file. A small JavaScript library
([MiniSearch](https://lucaong.github.io/minisearch/)) runs in the browser
and queries that file locally. No requests leave the user's machine after
the initial page load.

The index includes each post's title, body text, tags, language, and type,
so filters for all of those work out of the box.

## Configuration

The index file path is set in `bckt.yaml`:

```yaml
search:
  asset_path: /assets/search/search-index.json
  default_language: en
```

`default_language` is used to apply the right stemmer when indexing posts
that don't have an explicit `language:` field in their front matter.

## The search page

The search UI lives in `pages/search/index.html`. The bundled themes
include a fully styled search page — you can customise it like any other
template. The page itself is just HTML with a few `data-` attributes that
the search script hooks into; there's no framework or build step involved.

## What it costs

The index is a JSON file. For a typical blog with a few hundred posts it
stays well under 1 MB. The MiniSearch library adds about 30 KB gzipped.
Both are served as static files and cached aggressively by browsers, so
the cost for repeat visitors is effectively zero.
