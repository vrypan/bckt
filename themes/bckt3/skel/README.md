# Static Assets (`skel/`)

The `skel/` directory holds files that should be copied verbatim into the rendered
`html/` tree. During `bckt render` every file and sub-directory under `skel/`
is mirrored into the build output, preserving paths and timestamps where possible.
Use this area for CSS, JavaScript, fonts, favicons, and any other assets that do
not need template processing.

## Tips
- Keep global styles in `skel/style.css`; the default `base.html` already links to
  `/style.css`.
- Group related assets in folders, e.g. `skel/img/` for images or `skel/js/` for
  small client helpers.
- Because files are copied as-is, you can store generated bundles (Tailwind, Vite,
  etc.) here without additional configuration.

## Example
```
skel/
├── style.css
└── img/
    └── avatar.jpg
```

Refer to these files from templates using root-relative paths:
```html
<link rel="stylesheet" href="/style.css">
<img src="/img/avatar.jpg" alt="Avatar">
```

> **Heads-up:** delete this README from production builds so it isn’t copied into
> your published `html/` output.
