---
title: "You Can Write Long Posts Too"
slug: "long-post"
date: "2024-02-09T11:00:00Z"
tags:
  - bckt
  - writing
  - meta
type: post
language: en
abstract: "Microblog themes are designed for short notes, but they handle long-form posts just fine. Here is a tour of the Markdown features bckt supports."
attached:
  - harbour.jpg
  - rooftops.jpg
images:
  - harbour.jpg
  - rooftops.jpg
---

Microblog themes are optimised for short notes, but nothing stops you from writing a longer piece. This post exists to show that off — and to demonstrate the full range of Markdown features bckt supports along the way.

## Headings

You can use up to four levels of headings. Two or three is usually enough for a post. If you need more structure than that, consider whether you're writing a post or a document.

## Inline formatting

Text can be **bold**, *italic*, or ***both***. You can ~~strike things through~~, write `inline code`, and add [links to other pages](https://example.com).

## Blockquotes

> The best writing is rewriting.
>
> — E.B. White

Blockquotes work well for pulling out a key idea or attributing a source.

## Lists

Unordered:

- Start with an idea
- Write it badly
- Edit it until it's not

Ordered:

1. Draft without stopping
2. Wait a day
3. Cut the first paragraph
4. Read it aloud

## Code blocks

Fenced code blocks with language hints get syntax highlighting:

```python
def slugify(text):
    return text.lower().strip().replace(" ", "-")
```

```yaml
---
title: "My Post"
type: note
date: "2024-02-09T11:00:00Z"
tags:
  - example
---
```

## Tables

| Feature       | Supported | Notes                        |
|---------------|:---------:|------------------------------|
| Bold / italic | ✓         | Standard Markdown             |
| Tables        | ✓         | GFM-style                    |
| Footnotes     | ✓         | `[^1]` syntax                |
| Code blocks   | ✓         | With language hint            |
| Raw HTML      | ✓         | Passed through unchanged      |

## Images with captions

Images go in the `attached:` and `images:` front matter lists. The carousel above is rendered from those. For a captioned image inside the post body, use a figure:

<figure>
  <img src="harbour.jpg" alt="A harbour at dusk">
  <figcaption>The harbour. Late afternoon, no particular reason to be there.</figcaption>
</figure>

## Footnotes

bckt supports footnotes via the standard extended syntax.[^1] They render at the bottom of the post.

[^1]: Like this one. Useful for asides you don't want in the main flow of the text.

## Horizontal rules

Use three dashes on their own line to draw a separator:

---

That is most of what you need. The rest is just writing.
