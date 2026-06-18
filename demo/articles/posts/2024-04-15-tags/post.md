---
title: "Tags, Archives, and How bckt Organises Your Posts"
slug: "tags"
date: "2024-04-15T10:00:00Z"
tags:
  - bckt
  - features
abstract: "bckt automatically generates tag pages, year archives, and month archives from your post metadata — nothing to configure."
language: en
attached: []
---

bckt generates several kinds of archive pages automatically, all driven
by the metadata in your posts' front matter.

## Tags

Add a `tags:` list to any post:

```yaml
tags:
  - writing
  - tools
```

bckt builds a page at `/tags/writing/` and `/tags/tools/` listing all
posts with that tag, ordered by date. Each tag page also gets its own
RSS feed at `/rss-writing.xml` — but only if `writing` is listed
under `rss_tags:` in `bckt.yaml`. Tag feeds are opt-in.

Tag archive pages are rendered using the `tag.html` template, which you
can customise like any other template.

## Year and month archives

Posts are also indexed by date. bckt generates:

- `/2024/` — all posts from 2024
- `/2024/04/` — all posts from April 2024

These use the `archive_year.html` and `archive_month.html` templates.

## What drives it all

All of this comes from the front matter fields bckt reads at render time:

| Field    | Used for                          |
|----------|-----------------------------------|
| `date`   | Year/month archives, feed ordering |
| `tags`   | Tag archive pages and feeds        |
| `title`  | Archive listings, RSS              |
| `slug`   | Permalink construction             |

No database, no separate config file — the post itself is the source of
truth for how it gets indexed and linked.

## Incremental builds

bckt caches the output of each archive page. On subsequent renders it
only rebuilds the pages whose inputs have changed — so adding one new
post doesn't force a full rebuild of every tag archive.
