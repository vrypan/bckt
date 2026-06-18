---
title: "Themes: Installing, Switching, and Customising"
slug: "themes"
date: "2024-05-06T10:00:00Z"
tags:
  - bckt
  - features
abstract: "A walkthrough of bckt's theme system — what a theme contains, how to install one, and how to make it your own."
language: en
attached: []
---

A bckt theme bundles three things: templates, static assets, and
starter pages. Installing a theme gives you a working site layout
immediately; from there you customise as much or as little as you want.

## What a theme contains

```
themes/modern/
  templates/     ← MiniJinja templates for every page type
  skel/          ← CSS, JS, images — copied to html/ on render
  pages/         ← starter About and Search pages
```

`templates/` and `skel/` define the look. `pages/` seeds the project's
own `pages/` directory on `bckt init` and is never touched again.

## Installing a theme

Themes ship as `.zip` archives alongside the bckt binary. Install one
with:

```
bckt themes install modern
```

bckt finds `modern.zip` in the same directory as the binary (or in
`<prefix>/share/bckt/` for package manager installs) and extracts it
into `themes/modern/`.

You can also install from an explicit path:

```
bckt themes install ~/Downloads/custom-theme.zip
bckt themes install ~/projects/my-theme/
```

## Applying a theme

Installing puts the theme in `themes/` but doesn't activate it.
To apply it to the current project:

```
bckt themes use modern
```

This copies `templates/` and `skel/` from the theme into the project
root and updates `theme:` in `bckt.yaml`. Your `pages/` content is
left untouched.

## Starting fresh with a theme

When initialising a new project, pass `--theme` to install and apply
a theme in one step:

```
bckt init --theme modern --demo articles
```

## Bundled themes

bckt ships with six themes: `bckt3`, `micro`, `microx`, `modern`,
`plain`, and `rntz`. Each has a different aesthetic — `modern` and
`micro`/`microx` suit long-form articles and microblogs respectively;
`rntz` is a typographer's theme; `plain` is a minimal starting point
for building your own.

## Customising

Once a theme is applied, its files are yours. Edit `templates/base.html`
to change the navigation, tweak `skel/style.css` to adjust colours and
typography, add a partial for something the theme doesn't include. bckt
doesn't distinguish between "theme files" and "your files" — it just
renders what's there.
