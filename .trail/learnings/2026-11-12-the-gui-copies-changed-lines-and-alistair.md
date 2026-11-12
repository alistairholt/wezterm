---
title: The GUI copies changed lines and shapes them after releasing the terminal lock
author: Alistair
date: 2026-11-12
branch: claude/unlocked-paint
cost: { minutes: 1, tokens: 2473 }
id: l-25731460
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/render/pane.rs
  - lfucache/src/lib.rs
---

## Decided
Shaping and quad building run outside the pane's terminal lock. The lock covers only keying each visible line and copying the lines whose cached quads can't be used. Shaping CJK-heavy lines under the lock left the parser thread waiting on it most of the time. Measured when this was decided: `cat` of a 64 MB CJK file went from 20 to 91 MB/s, and ASCII, SGR and redraw CPU didn't change.

The shape hash is computed on the terminal's own line, under the lock, so the hash cached in that line's appdata survives between frames. Lines are drawn in screen order because a glyph that spills out of its cell is drawn over or under its neighbor depending on order.

## Tried and ruled out
- Copying every visible line with get_lines and hashing the copies (the agent's first experiment). The hash was cached on the copy and thrown away with it, so lines that changed after the switch were rehashed on every frame.

## Watch out
- The lock pass checks the quad cache with `peek`, so a line isn't counted as used twice. If a line drawn earlier in the frame evicts a later line's entry (only with about 320+ visible rows), that line is copied again under a short lock and drawn from the newer copy.
- Anything added to the per-line draw step must not read the terminal through the pane and expect it to match the copied line.

In the code:
- wezterm-gui/src/termwindow/render/pane.rs: LineRender::quad_key_for_line, cached_quad_is_usable, render_uncached, draw_prepared, PreparedLine
- lfucache/src/lib.rs: LfuCache::peek
