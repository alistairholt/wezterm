---
title: The cursor guide is a layer-0 quad drawn before the lines
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 20443 }
id: l-978e1985
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/render/pane.rs
---

## Decided
cursor_guide_color (iTerm2's cursor guide) is one filled_rectangle on layer 0 in paint_pane. It is drawn after the pane background and before the screen lines. Lines draw no background quad for cells with the default background (screen_line.rs skips bg_is_default), so text sits on the guide. Cells with their own background cover it, as in iTerm2. It uses background_rect's x and width, so it reaches the pane's edges like the background does. The row is the cursor's stable row minus the viewport top, and the guide is skipped when that is off screen.

## Tried and ruled out
A guide on layer 1 or 2 was not used: it would tint the glyphs as well as the row.

In the code: wezterm-gui/src/termwindow/render/pane.rs paint_pane, the block before the scrollbar.
