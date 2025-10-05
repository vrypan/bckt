# Hosting bckt Themes

You can share bckt themes by hosting a zip archive and letting users download it
with `bckt themes download` or `bckt init --theme-url`.

## Structure your archive

A theme archive should contain the usual directories:

```
templates/
skel/
pages/
```

and

```
theme.yaml
```

Zip up the folder so the theme lives at the root of the archive:

```bash
zip -r minimal-theme.zip templates skel pages
```

## Host the zip

Upload the archive to any HTTP(S) location (e.g. GitHub Releases, S3, your own
site) and note the public URL.

## Users download and install

```bash
bckt themes download theme --url https://example.com/minimal-theme.zip
bckt themes use mytheme
```

`--subdir` can be used if the zip contains extra path components, but when downloading from GitHub you can also append the base path to the `owner/repo` string (for example `owner/repo/themes`).

## GitHub convenience

To share a theme from a GitHub repo, tag a release and instruct users to run:

```bash
bckt themes download mytheme \
  --github your-name/your-theme/themes \
  --tag v1.0.0
```

`bckt init` accepts the same flags so a new project can bootstrap directly from a
remote theme.
