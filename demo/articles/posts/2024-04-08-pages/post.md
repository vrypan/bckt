---
title: "Standalone Pages and How They Work"
slug: "pages"
date: "2024-04-08T10:00:00Z"
tags:
  - bckt
  - features
abstract: "bckt's pages/ directory gives you arbitrary static pages alongside your posts — no routing configuration required."
language: en
attached: []
---

Not everything on a site is a post. You need an About page, maybe a
Projects page, a colophon. bckt handles these through the `pages/`
directory.

## The structure

Each subdirectory under `pages/` becomes a page at the corresponding
URL path:

```
pages/
  about/
    index.html      → /about/
  projects/
    index.html      → /projects/
  search/
    index.html      → /search/
```

Each `index.html` is a MiniJinja template. It has access to the same
template variables as any other page — `config`, `base_path`, and
anything else the theme exposes.

## Writing a page

A minimal page extends the theme's `base.html` and fills in the
`content` block:

```html
{% extends "base.html" %}
{% block content %}
<article class="page">
  <h1>About</h1>
  <p>This is where I write about things.</p>
</article>
{% endblock %}
```

For pages that need specialised layout or scripts — like the search
page — you can extend a more specific template instead:

```html
{% extends "search.html" %}
```

## How pages are seeded

Running `bckt init` copies the starter pages from the active theme
into your project's `pages/` directory. From that point the files are
yours — bckt never overwrites them, even when you switch themes with
`bckt themes use`. This is intentional: pages hold your content, not
the theme's.

## Adding a page to navigation

Navigation is part of the theme's `base.html` template. To add a new
page to the nav, edit `templates/base.html` and add a link:

```html
<a href="{{ base_path }}/projects/">Projects</a>
```

Because navigation is just HTML in a template, you have full control
over order, labels, and structure.
