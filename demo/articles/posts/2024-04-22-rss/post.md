---
title: "RSS in bckt: Automatic Feeds for Posts and Tags"
slug: "rss"
date: "2024-04-22T10:00:00Z"
tags:
  - bckt
  - features
abstract: "bckt generates RSS feeds for your whole site and for each tag — no plugins, no configuration."
language: en
attached: []
---

RSS is generated automatically. There's nothing to enable or configure —
every bckt site gets feeds from the first render.

## What gets a feed

- `/rss.xml` — all posts, newest first
- `/rss-<tag>.xml` — posts with that tag, newest first (opt-in via `rss_tags:` in `bckt.yaml`)

The number of items in each feed follows the same `homepage_posts`
setting in `bckt.yaml` that controls the home page listing.

## The feed template

Feeds are rendered from `templates/rss.xml`. It's a MiniJinja template
like any other, so you can edit it directly. The default template
produces valid RSS 2.0 with full post content in each item.

If you want Atom instead, you can replace or add a template — bckt
doesn't impose a feed format, it just renders whatever templates you
provide.

## Why RSS matters

RSS lets readers subscribe to your site without giving you their email
address and without depending on a social platform to surface your
posts. A feed reader like NetNewsWire, Reeder, or Miniflux checks
for new content on the reader's schedule, not yours. It's the most
durable subscription mechanism the web has.

Per-tag feeds go further: a reader interested only in your writing
about, say, photography can subscribe to `/rss-photography.xml`
and filter out everything else. That granularity is something email
newsletters and social feeds can't match.
