---
slug: "pages"
date: "2024-02-12T09:00:00Z"
tags:
  - bckt
type: note
language: en
attached: []
---

Standalone pages — like About and Search — live in `pages/`. Each
subdirectory becomes a page at that URL path.

Drop an `index.html` (a MiniJinja template) in `pages/about/` and it
renders to `/about/index.html`. No routing config, no special syntax.
