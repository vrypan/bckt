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
