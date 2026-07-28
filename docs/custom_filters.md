# Custom MiniJinja Filters

The bckt renderer wires theme-specific filters in `src/template/filters.rs`. These
filters extend the standard MiniJinja runtime with helpers that fit the
microblog theme.

## `format_date`

`format_date` formats RFC3339 timestamps into human-friendly strings. The
filter expects to receive a string such as `post.date_iso` (which already
contains an RFC3339 timestamp) and a format string that follows the familiar
`strftime` tokens.

```jinja
{{ post.date_iso | format_date("%a, %d %B %Y %H:%M") }}
```

Common tokens:

- `%Y` four-digit year
- `%m` numeric month (`01`-`12`)
- `%B` full month name
- `%d` day of month with leading zero
- `%H`, `%M`, `%S` 24-hour time, minutes, seconds
- `%a`, `%A` abbreviated and full weekday name
- `%b`, `%B` abbreviated and full month name
- `%R` (`%H:%M`), `%T` (`%H:%M:%S`), `%F` (`%Y-%m-%d`)

If a token is not supported, or the input is not RFC3339, the renderer aborts
with a descriptive error so template issues surface early during builds.

When formatting dates stored in front matter, prefer the provided
`post.date_iso` rather than `post.date` to ensure the filter receives the exact
RFC3339 timestamp.

# Template functions

In addition to filters, bckt registers a few global functions that templates
call directly (registered in `src/template/mod.rs`).

## `now`

`now([format])` returns the current UTC time as a formatted string. With no
argument it uses the site's configured `date_format`; pass `"RFC3339"` for an
RFC3339 timestamp, or any `time`-crate format description.

```jinja
{{ now() }}
{{ now("RFC3339") }}
```

## `atproto_tid`

`atproto_tid(date, slug)` returns a 13-character [atproto TID](https://atproto.com/specs/tid)
(record key) derived deterministically from a post's publication date and slug.
It exists to bridge a bckt blog to Bluesky (atproto): a separate poller can
mirror each post to a PDS at a **predictable** record key, and the page can link
to its own Bluesky post — both sides agree on the key without coordinating,
because bckt computes it once at build time and publishes it through the
templates.

Signature: `atproto_tid(date, slug)` → 13-char TID string over the sortable base32
alphabet `234567abcdefghijklmnopqrstuvwxyz`. The `date` argument must be an
RFC3339 timestamp (use `post.date_iso`); a non-RFC3339 value aborts the build
with a descriptive error.

Intended call sites:

```jinja
{# in a post template, e.g. to build the Bluesky URL #}
{{ atproto_tid(post.date_iso, post.slug) }}
```

```xml
<!-- in rss.xml, one per item, as a namespaced element. Add
     xmlns:atproto="..." to the <rss> root when wiring this up. -->
<atproto:rkey>{{ atproto_tid(item.date_iso, item.slug) }}</atproto:rkey>
```

**Immutability caveat.** The TID is derived only from `date` and `slug`.
Changing either on a post that has already been mirrored produces a *new* key,
and therefore a **second** Bluesky post rather than an update. Treat a post's
`date` and `slug` as immutable once it has been published.

**Freeze note.** The algorithm is intentionally fixed. bckt will not change the
hash or bit layout, because doing so would re-key every already-published post —
silently breaking the links embedded in already-deployed pages while the live
PDS records keep their old keys. (It is deliberately *not* byte-compatible with
`goat syntax tid generate`; nothing cross-checks the value, so bckt derives the
clock bits from a blake3 hash of the slug rather than matching goat's sha256.)
