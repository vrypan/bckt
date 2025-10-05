# bckt3 Theme Documentation

This directory gathers the theme guides referenced by the default `bckt3`
integration. Use it as the jumping-off point when customising layouts, assets,
filters, or the search experience.

All MiniJinja templates and static assets are editable; see the documents below
for detailed reference.

## Contents
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

## Themes are downloaded on demand

When you run `bckt init`, the CLI downloads the default `bckt3` theme from
[github.com/vrypan/bckt](https://github.com/vrypan/bckt) and unpacks it under
`themes/bckt3/`. 

If you prefer to start from a custom source you can override the defaults:

```bash
bckt init --theme-github your-name/your-theme --theme-subdir themes/minimal
# or
bckt init --theme-url https://example.com/theme.zip --theme-name mytheme
```

See [theme-hosting.md](theme-hosting.md) for details on packaging themes and
hosting archives.
