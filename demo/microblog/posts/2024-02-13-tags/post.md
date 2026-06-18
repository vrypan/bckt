---
slug: "tags"
date: "2024-02-13T09:00:00Z"
tags:
  - bckt
type: note
language: en
attached: []
---

Tag archive pages are generated automatically. Add a `tags:` list to
any post's front matter and bckt builds a page at `/tags/<tag>/` with
all posts carrying that tag. No configuration needed.

Tag RSS feeds are opt-in: add `rss_tags:` to `bckt.yaml` listing the
tags you want feeds for, and bckt generates `/rss-<tag>.xml` for
each one.

Year and month archives are generated the same way.
