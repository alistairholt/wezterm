---
title: "The line quad cache stores quads in flat vectors, not one heap box per quad"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 4, tokens: 28197 }
id: l-730de121
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - wezterm-gui/src/termwindow/render/mod.rs
  - wezterm-gui/src/termwindow/render/shape_hash.rs
  - config/src/config.rs
  - wezterm-gui/src/quad.rs
---

## Decided
HeapQuadAllocator keeps each layer as a plain vector of compact quads. It used to hold one heap box per quad, which meant a malloc for every glyph when a line was drawn and a free for every glyph when its cache entry was replaced.

The old comment justified the boxes by MB-sized arrays that would be costly to reallocate. That doesn't apply: there is one allocator per cached line, holding a few hundred quads of 84 bytes each, so growing the vector is cheap.

## Measured
Release build, Time Profiler, 200x60:
- Box new plus drop: 36 → 0 ms/s while scrolling, 32 → 1 ms/s on a full redraw.
- Main thread CPU: 229 → 198 ms/s (scroll) and 194 → 187 ms/s (redraw).

## Watch out
malloc and free still take 40–50 ms/s on the main thread from other callers, so they are worth chasing next.

In the code: wezterm-gui/src/quad.rs (HeapQuadAllocator, BoxedQuad).
