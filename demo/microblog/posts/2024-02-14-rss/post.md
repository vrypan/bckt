---
slug: "rss"
date: "2024-02-14T09:00:00Z"
tags:
  - bckt
type: note
language: en
attached: []
---

There's an RSS feed at [`/rss.xml`](/rss.xml) — no configuration
required.

Tag feeds are opt-in: add `rss_tags:` to `bckt.yaml` to list the tags
you want feeds for, and bckt generates `/rss-<tag>.xml` for each,
so readers can subscribe to just the topics they care about.
