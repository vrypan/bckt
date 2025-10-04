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

## Working With Themes

The default theme can be found in `themes/bckt3/`. 

When you ran `bckt init`, `pages/`, `themes/` and `skel/` were copied from
`themes/bckt3/*`.

You can clone it as a starting point for your own design:

```bash
cp -r themes/bckt3 themes/mytheme
bckt themes use mytheme
```

> [!WARNING] 
> **Heads-up:** `bckt themes use` overwrites existing templates with the selected
> theme. Commit or copy any local changes before switching.

