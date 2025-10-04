# Static Assets (`skel/`)

The `skel/` directory stores files that should be copied verbatim into the
rendered `html/` tree. During `bckt render`, every file and subdirectory inside
`skel/` is mirrored to the output while preserving paths.

Use this area for CSS bundles, JavaScript, fonts, favicons, images, or any
other assets that do not require template expansion.

## Recommendations
- Keep global styles in `skel/style.css`; the default `base.html` already
  references `/style.css`.
- Group assets logically, e.g. `skel/img/` for images or `skel/js/` for client
  helpers.
- Generated bundles from tools like Tailwind or Vite can be placed directly in
  `skel/` and will be copied untouched.

## Example Layout
```
skel/
├── style.css
└── img/
    └── avatar.jpg
```

Reference these assets from templates using root-relative paths:

```html
<link rel="stylesheet" href="/style.css">
<img src="/img/avatar.jpg" alt="Avatar">
```
