# Pages Directory

The `pages/` directory contains standalone documents that are rendered with the
same MiniJinja environment as the rest of the theme. Use it for static content
such as an About page, Contact information, or a custom error page.

## Rendering Rules
- Files ending in `.html` are processed as templates, so you can extend
  `base.html` and reuse the theme blocks.
- Other file types (for example `.xml`, `.txt`) are copied verbatim, which is
  useful for robots.txt or other metadata documents.
- Nested folders become part of the output path. A source file at
  `pages/about/index.html` publishes to `/about/index.html` in the rendered
  output.

## Example Page Template
```jinja
{% extends "base.html" %}

{% block page_title %}About · {{ config.title }}{% endblock %}

{% block content %}
  <section class="page">
    <h1>About</h1>
    <p>I write about dev tooling, static sites, and the occasional side project.</p>
  </section>
{% endblock %}
```

Run `bckt render` and the page appears at `/about/`. Because it extends
`base.html`, it automatically inherits global navigation, metadata, and theme
styling.
