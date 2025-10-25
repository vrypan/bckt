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
- `base_url` — site base URL without trailing slash (e.g., `https://example.com/blog`).
- `base_path` — path component of `base_url` without trailing slash (e.g., `/blog`), empty string for root deployments.
- `posts` — list of `PostSummary` objects (varies by view).
- `pagination` — pagination metadata where applicable.
- `tag`, `year`, `month` — extra values specific to tag or archive templates.

#### Using base_url vs base_path

**Use `base_url` for:**
- Absolute URLs in RSS feeds, canonical links, and Open Graph tags
- External references that need the full domain

```jinja
<!-- RSS feed -->
<link>{{ base_url }}{{ item.permalink }}</link>

<!-- Canonical URL -->
<link rel="canonical" href="{{ base_url }}{{ post.permalink }}">
```

**Use `base_path` for:**
- Internal navigation links
- Asset references (CSS, JS, images)
- Links to other pages within your site

```jinja
<!-- Navigation -->
<a href="{{ base_path }}/">Home</a>
<a href="{{ base_path }}/search/">Search</a>

<!-- Assets -->
<link rel="stylesheet" href="{{ base_path }}/assets/css/style.css">

<!-- Post images when referenced from other pages -->
<img src="{{ base_path }}{{ post.permalink }}cover.jpg">
```

**Note:** Within individual post pages, attached images use relative paths automatically and work regardless of `base_path`.

#### PostSummary and PostTemplate Objects

Both `PostSummary` (used in listings and RSS) and `PostTemplate` (used in individual post pages) expose:

- `title`, `slug`, `permalink` — basic post identification
- `date`, `date_iso` — formatted date and ISO 8601 timestamp
- `tags` — array of tag strings
- `body`, `excerpt` — HTML content and excerpt
- `attachments` — HashMap of attached files with metadata (see below)
- All custom frontmatter fields via the flattened `extra` map

#### Attachment Metadata

Each post exposes an `attachments` map where keys are file paths and values contain:
- `size` — file size in bytes
- `mime_type` — MIME type (e.g., `image/png`, `application/pdf`)

**Direct lookup:**
```jinja
{% if post.image %}
  {% set att = post.attachments[post.image] %}
  <img src="{{ post.image }}" alt="Size: {{ att.size }} bytes">
{% endif %}
```

**Loop through all attachments:**
```jinja
{% for path, att in post.attachments | items %}
  <a href="{{ path }}">{{ path }}</a> ({{ att.mime_type }}, {{ att.size }} bytes)
{% endfor %}
```

**RSS enclosures:**
```xml
{% for path, att in item.attachments | items %}
  <enclosure url="{{ base_url }}{{ item.permalink }}{{ path }}"
             type="{{ att.mime_type }}"
             length="{{ att.size }}"/>
{% endfor %}
```

Refer to the existing templates for patterns and helper classes you can reuse
when building custom layouts or partials.
