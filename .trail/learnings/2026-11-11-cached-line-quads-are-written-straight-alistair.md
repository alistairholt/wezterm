---
title: Cached line quads are written straight into the GPU vertex buffer
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 5, tokens: 44709 }
id: l-9791ea79
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - wezterm-char-props/src/emoji.rs
  - wezterm-gui/src/quad.rs
  - wezterm-gui/src/renderstate.rs
  - wezterm-gui/src/termwindow/render/pane.rs
---

## Decided
When a line comes from the line quad cache, HeapQuadAllocator::apply_to hands each layer's compact quads to extend_with_quads, and the GPU side (MappedQuads) writes the four vertices of each quad directly into the shared Metal buffer. Before, each quad was expanded into a 272-byte stack array (Default, then the Quad setters) and copied in with copy_from_slice, one call per quad.

Measured (Time Profiler, 15 s, 200x60, max_fps 120, frame counts within 2%): the apply step under render_line went from 684 to 416 samples on a full-screen redraw (about 18 ms/s); render_line 2522 -> 2290.

## Watch out
- extend_with_quads skips the whole batch when it doesn't fit, where extend_with copied quad by quad. Both still advance `next`, so the end of the render pass sees the overflow and re-renders with a bigger buffer; the partial copy was never shown.
- What's left (write_vertices) is mostly the memory writes themselves: about 2 quads per cell, 272 bytes each. Shrinking it means a smaller vertex format or instancing (one 84-byte quad per instance), which needs shader changes.

In the code: wezterm-gui/src/quad.rs (BoxedQuad::write_vertices, extend_with_quads, test boxed_quad_vertices_match_quad_setters), wezterm-gui/src/renderstate.rs (MappedQuads::extend_with_quads).
