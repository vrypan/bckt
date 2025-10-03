# Templates (`templates/`)

Files in this directory are rendered with [MiniJinja](https://github.com/mitsuhiko/minijinja)
when `bucket3 render` runs. They define the layout for posts, list pages, tag
indexes, archives, and any custom pages you add.

## Key Files
- `base.html` — global shell with the `<head>` section and navigation.
- `post.html` — layout for individual posts built from Markdown or HTML sources.
- `index.html` — homepage feed with pagination context.
- `tag.html`, `archive_year.html`, `archive_month.html` — list views for tags and archives.
- `rss.xml` — Jinja-powered XML template for the RSS feed.

## Extending the Theme
Create new templates by extending `base.html` and overriding the blocks you need:
```jinja
{% extends "base.html" %}

{% block page_title %}About · {{ config.title }}{% endblock %}

{% block content %}
  <article class="page">
    <h1>About</h1>
    <p>This site runs on bucket3 and uses the bckt3 theme.</p>
  </article>
{% endblock %}
```

### Available Context
Most templates receive:
- `config` — the active `bucket3.yaml` values.
- `feed_url` — absolute URL to the generated RSS feed.
- `posts`, `pagination`, `tag`, or `archive` objects depending on the view.

Inspect the existing templates for patterns you can copy when building custom
layouts or partials.
