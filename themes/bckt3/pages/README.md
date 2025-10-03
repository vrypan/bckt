# Custom Pages (`pages/`)

Any files placed under `pages/` are rendered into the final `html/` tree using
the same MiniJinja environment as the rest of the theme. This is the right spot
for static content such as an About page, Contact details, or a custom 404.

## How Rendering Works
- `.html` files are parsed as templates, so you can extend `base.html` and reuse
  existing blocks.
- Other file types (e.g. `.xml`, `.txt`) are copied through without template
  expansion, which is handy for things like `robots.txt`.
- Nested folders become part of the output path. For example, `pages/about/index.html`
  renders to `/about/index.html` in the site output.

## Example Page
Create `pages/about/index.html` with:
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

After running `bckt render`, the resulting page is available at
`/about/index.html` and automatically picks up the theme styling.

> **Heads-up:** remove this README before deploying so it doesn’t ship as part
> of your generated site.
