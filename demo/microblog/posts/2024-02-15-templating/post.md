---
slug: "templating"
date: "2024-02-15T09:00:00Z"
tags:
  - bckt
type: note
language: en
attached: []
---

Templates live in `templates/` and use
[MiniJinja](https://github.com/mitsuhiko/minijinja) — a Rust
implementation of the Jinja2 syntax. Every post, index page, archive,
and feed is rendered from a template you can edit.

`base.html` defines the overall layout; individual templates extend it
and override blocks. bckt watches for template changes in dev mode and
rebuilds automatically.
