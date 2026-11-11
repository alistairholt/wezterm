---
title: "The line quad cache builds each line in a reused scratch buffer, then keeps a compact copy"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 5, tokens: 36899 }
id: l-f1a01524
scope: repo
status: active
replaces: l-730de121
src: agent
touches:
  - docs/changelog.md
  - lfucache/src/lib.rs
  - wezterm-gui/src/glyphcache.rs
  - wezterm-gui/src/quad.rs
  - wezterm-gui/src/stats.rs
  - wezterm-gui/src/termwindow/render/mod.rs
  - wezterm-gui/src/termwindow/render/screen_line.rs
  - wezterm-gui/src/termwindow/render/pane.rs
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
- HeapQuadAllocator keeps each layer as a plain vector of compact quads (BoxedQuad, 84 bytes). It used to hold one heap box per quad, which meant a malloc for every glyph when a line was drawn and a free for every glyph when its cache entry was replaced.
- Each line is built into a scratch HeapQuadAllocator that TermWindow keeps (`line_quad_scratch`) and clears per line. `to_compact` then copies it into exact-size vectors for line_quad_cache: one allocation per layer and no spare capacity held in the cache.

## Tried and ruled out
Building straight into a fresh vector per line. Growing from empty to a few hundred quads reallocates about 8 times per layer: 7 ms/s while scrolling and 6.5 ms/s on a full redraw. Reserving up front would have left spare capacity in every cached line.

## Measured
Release build, Time Profiler, 200x60, about 900 frames in 15s for each run:
- Box per quad → flat vector: box new plus drop went 36 → 0 ms/s (scroll).
- Fresh vector → scratch plus compact copy: allocations under render_line went 27.9 → 20.1 ms/s (scroll) and 8.9 → 2.9 ms/s (redraw). Main thread went 217 → 205 and 209 → 192 ms/s.

## Watch out
If render_screen_line fails, the scratch buffer is dropped and the next line starts from an empty default. That only costs the reallocations again.

In the code: wezterm-gui/src/quad.rs (HeapQuadAllocator), wezterm-gui/src/termwindow/render/pane.rs (render_line).
