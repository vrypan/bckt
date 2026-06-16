# bckt3 Theme Documentation

This directory gathers the theme guides referenced by the default `bckt3`
integration. Use it as the jumping-off point when customising layouts, assets,
filters, or the search experience.

All MiniJinja templates and static assets are editable; see the documents below
for detailed reference.

## Contents
- [Posts and Content](posts.md) — organizing posts, frontmatter, attached files,
  and ignoring directories with `.bcktignore`.
- [Pages Directory](pages.md) — how `pages/` files render and how to build
dedicated pages.
- [Templates Overview](templates.md) — anatomy of core templates and available
context variables.
- [Static Assets](static-assets.md) — managing files under `skel/`.
- [Custom Filters](custom_filters.md) — theme-specific MiniJinja helpers such as
  `format_date`.
- [Search Integration](search.md) — client-side search requirements and build
integration.
- [Hosting Themes](theme-hosting.md) — package and distribute reusable theme zips.

## Themes are installed from local archives

When you run `bckt init`, the CLI installs the default `bckt3` theme by locating
`bckt3.zip` on the theme search path (`BCKT_THEME_PATH`, then the directory
containing the `bckt` executable) and unpacking it under `themes/bckt3/`.

You can start from a different archive or a named theme instead:

```bash
bckt init --theme path/to/your-theme.zip
# or, resolved as <name>.zip via the search path
bckt init --theme your-theme
```

See [theme-hosting.md](theme-hosting.md) for details on packaging and sharing
theme archives.
