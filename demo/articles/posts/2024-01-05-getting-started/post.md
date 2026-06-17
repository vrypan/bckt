---
title: "Getting Started with bckt"
slug: "getting-started"
date: "2024-01-05T10:00:00Z"
tags:
  - bckt
  - tutorial
abstract: "A quick tour of bckt — what it is, how it works, and how to set up your first site."
language: en
attached: []
---

`bckt` is a static site generator built for blogs. It takes your Markdown posts and turns them into a fast, dependency-free website you can host anywhere.

## What you need

- The `bckt` binary (download from the releases page)
- A terminal
- Your writing

That is the full list.

## Setting up a project

Run `bckt init` in a new directory. It creates the scaffolding and installs a default theme:

```
$ mkdir myblog && cd myblog
$ bckt init --theme modern
Initialized project with theme 'modern'
```

The directory now looks like this:

```
myblog/
  posts/          ← your content goes here
  templates/      ← theme HTML templates
  skel/           ← theme static assets
  pages/          ← standalone pages (about, search, …)
  themes/         ← installed themes
  bckt.yaml       ← site configuration
```

## Writing a post

Each post lives in its own directory under `posts/`. Create a directory, add a `post.md`, and start writing:

```
posts/
  my-first-post/
    post.md
```

The front matter at the top of `post.md` sets the metadata:

```yaml
---
title: "My First Post"
slug: "my-first-post"
date: "2024-01-05T10:00:00Z"
tags:
  - writing
---

Your content here.
```

## Building the site

```
$ bckt render
```

The output lands in `html/`. Point any static file host at that directory and you are done.

## Previewing locally

```
$ bckt dev
```

Opens a local server at `http://127.0.0.1:4000` with live reload on changes.
