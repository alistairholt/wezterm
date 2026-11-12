---
title: "The Metal layer keeps two drawables, not three, and still holds 120fps and live resize"
author: Alistair
date: 2026-11-12
branch: claude/memory-throughput
cost: { minutes: 7, tokens: 30340 }
id: l-f7e8aa4e
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - wezterm-gui/src/termwindow/metal_present.rs
---

## Decided
The CAMetalLayer keeps at most two drawables. Each is a full-window BGRA texture, so the third cost one more window-sized texture per window. Frames are paced to the display, so a third buffer bought no frames.

## Watch out
- Checked on a 120Hz ProMotion screen: a full-screen redraw loop held 120fps with both counts. With the built-in resize driver (120 steps, 8 ms apart), there were 128 paints with three drawables and 130 with two, and per-paint time was the same.
- If frames start dropping, for example from slow GPU work on a big window, try three drawables again before anything else. With two, a late frame stalls in nextDrawable.

In the code:
- wezterm-gui/src/termwindow/metal_present.rs: set_maximum_drawable_count(2)
- Resize check: WEZTERM_DEBUG_RESIZE="3000;60,8,-5,-3,300;60,8,5,3,300", count paint_impl debug lines between debug-resize begin and end
