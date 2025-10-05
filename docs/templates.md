# Templates Overview

Theme templates live under `templates/` and are rendered with
[MiniJinja](https://github.com/mitsuhiko/minijinja) during `bckt render`.
They define page chrome, post layouts, archive listings, and any custom
structures you add.

## Core Templates
- `base.html` — global shell containing the `<head>` metadata, site navigation,
  and shared blocks. Most other templates extend this file.
- `post.html` — default layout for individual posts sourced from Markdown or
  HTML files. Variants like `post-farcaster.html` override the experience for
  specific post types.
- `index.html` — homepage feed that receives a `posts` collection and a
  `pagination` object.
- `tag.html`, `archive_year.html`, `archive_month.html` — list views for tags
  and archives.
- `rss.xml` — MiniJinja-driven XML template used to generate the RSS feed.

## Extending the Theme
Create new views by extending `base.html` and overriding the blocks you need:

```jinja
{% extends "base.html" %}

{% block page_title %}About · {{ config.title }}{% endblock %}

{% block content %}
  <article class="page">
    <h1>About</h1>
    <p>This site runs on bckt using the bckt3 theme.</p>
  </article>
{% endblock %}
```

### Context Reference
Most templates receive:

- `config` — parsed values from `bckt.yaml` (including `config.extra`).
- `base_url` — site base URL that always ends with a trailing slash.
- `posts` — list of `PostSummary` objects (varies by view).
- `pagination` — pagination metadata where applicable.
- `tag`, `year`, `month` — extra values specific to tag or archive templates.

Refer to the existing templates for patterns and helper classes you can reuse
when building custom layouts or partials.
