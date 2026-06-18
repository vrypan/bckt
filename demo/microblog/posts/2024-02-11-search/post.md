---
slug: "search"
date: "2024-02-11T09:00:00Z"
tags:
  - bckt
type: note
language: en
attached: []
---

Search is built in. No external service, no API key — bckt generates a
local index at render time and ships a small client-side script that
queries it.

See it working on the [Search](/search/) page. The index path is
configurable in `bckt.yaml` under `search.asset_path`.
