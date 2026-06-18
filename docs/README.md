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

## Building themes with an AI agent

[`themes/AGENTS.md`](https://github.com/vrypan/bckt/blob/main/themes/AGENTS.md)
is a guidance file for AI coding agents — Claude Code, Cursor, Copilot, and
similar tools. When asking an agent to build or modify a theme, start your
prompt with something like:

> Read https://github.com/vrypan/bckt/blob/main/themes/AGENTS.md and create
> a theme that resembles https://example.com. Also check `themes/*` to see
> how the existing themes are structured.

The file covers the apply-and-rebuild loop, MiniJinja templating, context
variables, pagination conventions, and static asset requirements — everything
an agent needs to produce a working theme without having to guess at bckt's
conventions.

## Themes are installed from local archives

When you run `bckt init`, the CLI installs the default `bckt3` theme by locating
`bckt3.zip` on the theme search path (`BCKT_SHARE_PATH`, then the directory
containing the `bckt` executable) and unpacking it under `themes/bckt3/`.

You can start from a different archive or a named theme instead:

```bash
bckt init --theme path/to/your-theme.zip
# or, resolved as <name>.zip via the search path
bckt init --theme your-theme
```

See [theme-hosting.md](theme-hosting.md) for details on packaging and sharing
theme archives.
