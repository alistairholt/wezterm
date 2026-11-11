---
title: "The mux client's line cache must keep its size limit when a resize marks lines stale"
author: Alistair
date: 2026-11-11
branch: claude/client-line-cache
cost: { minutes: 3, tokens: 13514 }
id: l-e7ed777b
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - wezterm-client/src/domain.rs
  - wezterm-mux-server-impl/src/dispatch.rs
  - wezterm-mux-server-impl/src/sessionhandler.rs
  - wezterm-client/src/pane/renderable.rs
---

## Decided
RenderableInner.lines is an LruCache capped at scrollback_lines. make_all_stale, called on every resize, rebuilt it with LruCache::unbounded(), so after the first resize the client kept every line a pane printed. With `yes` in a background mux tab the GUI grew ~11 MB/s (213 -> 541 MB in 40 s); with the cap kept it stays at ~120 MB.

## Watch out
Rebuilding an LruCache with pop_lru + put keeps the order; give the new cache lines.cap(), never unbounded().

In the code: wezterm-client/src/pane/renderable.rs make_all_stale (free function, tested).
