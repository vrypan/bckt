---
title: "Why Static Sites Still Make Sense"
slug: "static-sites"
date: "2024-02-12T14:30:00Z"
tags:
  - web
  - publishing
abstract: "The case for static sites in 2024 — not as a nostalgia trip, but as an engineering argument."
language: en
attached: []
---

There is a recurring conversation in web development where someone discovers static sites and frames it as a return to simplicity, as if the whole industry took a wrong turn and is now course-correcting. That framing is mostly wrong, but the conclusion — that static sites are worth using — is mostly right.

## What static actually means

A static site is one where the server sends the same pre-built file to every visitor. There is no database query, no template rendering at request time, no session state. The file is the response.

This is not a limitation. It is a constraint, and constraints are useful.

## The performance argument

Every dynamic page has a minimum cost: the time it takes to run your application code, query a database, render a template, and stream the result. On a well-tuned stack with warm caches, that cost can be very low. But it is never zero, and under load it compounds.

A static file served from a CDN has no such floor. The response time is dominated by network latency, which is already as small as physics allows.

For a blog, a documentation site, a portfolio — anything where the content changes infrequently and the reader is just reading — this is an obvious win.

## The operational argument

Dynamic applications need running infrastructure: web servers, application servers, databases, background workers. Each of those is something that can go down, something that needs patches, something that needs capacity planning.

A static site needs none of this. It is a directory of files. You can host it on S3, on GitHub Pages, on a $5 VPS, on a USB drive if you want to get creative. The operational surface area is close to zero.

## Where it does not work

The constraint matters. If your site has user accounts, real-time data, search that has to hit a live index, or any server-side personalisation, static generation does not cover those cases on its own.

The common pattern is to pair a static frontend with a small set of APIs for the dynamic parts. This is reasonable, but it means your site is no longer fully static — you are back to running infrastructure, just less of it.

## The right question

The question is not "static or dynamic?" but "which parts of this actually need to be dynamic?" For most publishing use cases, the answer is: very few. The content is static. The layout is static. Search can be client-side. Comments can be outsourced or dropped entirely.

If you start from that question and work backwards, you often end up with a static site plus a handful of calls to external services, which is simpler and cheaper to run than a full application stack.

That is the real argument for static sites — not nostalgia, not minimalism for its own sake, but matching the tool to the actual requirements.
