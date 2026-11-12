---
title: "Line caches are trimmed by recency to the lines on screen, not by lowering their size"
author: Alistair
date: 2026-11-12
branch: claude/memory-throughput
cost: { minutes: 8, tokens: 42345 }
id: l-daa35513
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - lfucache/src/lib.rs
  - wezterm-gui/src/termwindow/render/paint.rs
---

## Decided
After each frame the line quad cache keeps 2x and the line shape cache 4x the rows on screen (plus 64), dropping the least recently used entries. Every line drawn in a frame was just used, so live lines always survive. The quad cache key includes the line's position, so lines that scroll away never hit again; before this both caches filled to 1024 lines of them (5-15 MB after a burst of output). GUI CPU on full redraw and on scrolling output was unchanged.

## Tried and ruled out
- Lowering the configured cache sizes: the caches evict by lowest count, so with a tight cap the fresh lines of the current frame (count 0) evict each other while old, once-hot lines that scrolled away stay. Trimming by recency avoids that.

## Watch out
- The remaining growth after output (~18 MB) is Metal drawables: macOS allocates all three once painting is continuous. Cutting to two is a frame pacing question, not a cache one.

In the code: lfucache/src/lib.rs trim_least_recent (test trim_keeps_most_recent); wezterm-gui/src/termwindow/render/paint.rs trim_line_caches, called at the end of paint_pass.
