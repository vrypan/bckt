# Sharing bckt Themes

bckt themes are distributed as plain `.zip` archives. Share the archive however
you like (a release page, your own site, email), and users install it from a
local file with `bckt themes install`.

## Structure your archive

A theme archive should contain the theme directories at its **root**:

```
templates/
skel/
pages/
```

`templates/` and `skel/` define the look (skel holds static files such as CSS
and JavaScript, e.g. `skel/assets/js/`). `pages/` holds optional starter pages.

Zip the contents so they live at the root of the archive:

```bash
cd my-theme
zip -r ../my-theme.zip templates skel pages
```

The archive's file name (without `.zip`) becomes the default theme name on
install, so name it after the theme (for example `my-theme.zip`).

## Users install the theme

```bash
bckt themes install path/to/my-theme.zip
bckt themes use my-theme
```

`bckt themes install` extracts the archive into `themes/<name>/` (override the
name with `--name`, overwrite an existing one with `--force`). `bckt themes use`
copies `templates/` and `skel/` into the project and records the theme in
`bckt.yaml`; your `pages/` are left untouched.

## Theme search path and `bckt init`

`bckt init` installs a default theme (`bckt3`) by resolving the name across the
theme search path, preferring `<name>.zip` and falling back to a `<name>/`
directory. The search path is, in order:

1. directories listed in the `BCKT_THEME_PATH` environment variable;
2. the directory containing the `bckt` executable (so a distribution bundle or
   an extracted tarball can ship `bckt` and `bckt3.zip` side by side);
3. `../share/bckt` relative to the resolved executable — the conventional
   `<prefix>/bin` + `<prefix>/share/<pkg>` layout.

Entry 3 means package-manager installs are discovered automatically: a Homebrew
formula that installs the theme under `share/bckt` (e.g. `share.install "bckt3"`)
is found with no configuration, because `bckt`'s binary resolves into the Cellar
and `../share/bckt` points back at the installed theme. `BCKT_THEME_PATH` remains
available as an override.

You can also initialise from an explicit archive, directory, or named theme:

```bash
bckt init --theme path/to/my-theme.zip
bckt init --theme path/to/my-theme   # a theme directory
bckt init --theme my-theme           # resolved via the search path
```
