---
title: "Templating with MiniJinja"
slug: "templating"
date: "2024-04-29T10:00:00Z"
tags:
  - bckt
  - features
abstract: "bckt uses MiniJinja for all templates — a fast, Jinja2-compatible engine embedded in the binary. Here's how the template system is structured."
language: en
attached: []
---

Every page bckt renders — posts, index, archives, feeds, standalone
pages — comes from a MiniJinja template. If you've used Jinja2 or
Django templates, the syntax is familiar. If not, it's easy to pick up.

## Template inheritance

Templates are organised around inheritance. `base.html` defines the
overall page structure — `<html>`, `<head>`, navigation, footer — and
declares named blocks that child templates can fill in:

```html
<!doctype html>
<html lang="{{ config.default_language | default('en') }}">
<head>
  <title>{% block page_title %}{{ config.title }}{% endblock %}</title>
  ...
</head>
<body>
  {% block content %}{% endblock %}
</body>
</html>
```

A post template extends `base.html` and fills the `content` block:

```html
{% extends "base.html" %}
{% block content %}
<article>
  {% if post.title %}<h1>{{ post.title }}</h1>{% endif %}
  <div class="post-body">{{ post.body | safe }}</div>
</article>
{% endblock %}
```

## Variables available in templates

| Variable      | Contents                                      |
|---------------|-----------------------------------------------|
| `config`      | Values from `bckt.yaml`                       |
| `post`        | Current post (on post pages)                  |
| `posts`       | List of posts (on index and archive pages)    |
| `base_path`   | Root-relative URL prefix                      |
| `tag`         | Current tag name (on tag pages)               |

Post objects expose `title`, `slug`, `date_iso`, `permalink`, `body`,
`tags`, `language`, `type`, `images`, `attached`, and any extra fields
you add to the front matter.

## Partials

Reusable snippets live in `templates/partials/` and are included with:

```html
{% include "partials/sidebar.html" %}
```

Themes use partials for things like post summaries, carousels, and
search results — pieces that appear in multiple templates.

## Live reload in dev mode

`bckt dev` watches `templates/` for changes. When you save a template,
bckt rebuilds the affected pages and the browser reloads. The feedback
loop is fast enough to work directly in the template without a separate
preview step.
