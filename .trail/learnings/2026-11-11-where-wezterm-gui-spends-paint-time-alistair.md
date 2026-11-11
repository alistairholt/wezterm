---
title: "Where wezterm-gui spends paint time on the Metal path, and how to profile it"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 14, tokens: 172468 }
id: l-8d722fd0
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/renderstate.rs
  - wezterm-gui/src/termwindow/render/draw.rs
  - config/src/config.rs
  - wezterm-surface/src/hyperlink.rs
  - wezterm-gui/src/quad.rs
---

## Decided
Measured before changing anything (release build, Time Profiler, local pane, 200x60, front_end WebGpu, 60 fps reached). Per frame, main thread: paint_impl about 3.5 ms (scrolling truecolor stream) to 4.5 ms (full-screen redraw every frame). Biggest pieces:
- Vertex buffer recreate (wgpu create_buffer zero-fills a full-capacity staging buffer each frame): 0.45-0.6 ms.
- Implicit hyperlink scanning: 0.55-0.9 ms. The CLOSING_PARENTHESIS default rule has a lookahead, so fancy_regex runs its backtracking VM over every changed line.
- Line::compute_shape_hash (SipHash128 over cells): 0.35-0.9 ms.
- BoxedQuad malloc/free in the line quad cache: 0.3-0.4 ms.
- metrics histograms: under 0.1 ms, so not worth fixing first.
MetalPresenter acquire + present is about 1 ms per frame and nextDrawable is negligible.

## Watch out
- Release profile has no debug info; build with CARGO_PROFILE_RELEASE_DEBUG=line-tables-only (no file change) or the profile is unreadable.
- Frame count comes from WEZTERM_LOG='warn,wezterm_gui::termwindow::metal_present=trace' and counting "metal_present: step" lines.
- Unset WEZTERM_* env vars and pass start --always-new-process, or the GUI reuses a running instance.

In the code:
- wezterm-gui/src/renderstate.rs recreate; wezterm-gui/src/termwindow/render/draw.rs call_draw_webgpu
- config/src/config.rs default_hyperlink_rules; wezterm-surface/src/hyperlink.rs CLOSING_PARENTHESIS_HYPERLINK_PATTERN
- wezterm-gui/src/quad.rs HeapQuadAllocator
