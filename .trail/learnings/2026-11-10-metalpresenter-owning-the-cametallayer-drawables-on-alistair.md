---
title: "MetalPresenter: owning the CAMetalLayer drawables on the WebGpu path"
author: Alistair
date: 2026-11-10
branch: metal-presenter
cost: { minutes: 5, tokens: 12233 }
id: l-69d9766f
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/metal_present.rs
  - wezterm-gui/src/termwindow/render/draw.rs
  - wezterm-gui/src/termwindow/webgpu.rs
  - window/src/os/macos/window.rs
---

## Decided
- The drawable is sized from TermWindow.dimensions (what the projection is built for), not from view bounds x scale. If they ever lag the view, a bounds-sized drawable would stretch the frame inside it; a dimensions-sized one shows 1:1 pinned top-left.
- Sync vs async is decided against the size of the last drawable we got (MetalPresenter.last_size), not the layer's drawableSize, because Core Animation can change drawableSize itself when bounds or contentsScale change, which would hide a size change.
- contentsScale = backingScaleFactor only when front_end is WebGpu (WindowView::presents_with_metal). The OpenGL paths still rely on 1.0 plus the default stretching gravity.
- did_end_live_resize re-sends Resized so TermWindow.live_resizing goes back to false; otherwise every later frame would present synchronously.

## Watch out
- A texture made with create_texture_from_hal counts as initialized in wgpu, so a frame with no vertices must still run a clear pass or it shows garbage. call_draw_webgpu adds one when nothing cleared.
- With the live-resize paint bypass in draw_rect, max_fps never skips a live-resize step, so the gravity can't be seen in a live drag. WEZTERM_DEBUG_RESIZE_NOT_LIVE=1 runs the debug driver outside a live resize to show undrawn steps.
- Which gravity name is the visual top depends on layer flipping ("top" is max Y). The presenter picks from contentsAreFlipped and logs "metal_present: geometryFlipped=... gravity a -> b" at info; check it on screen.
- objc msg_send in wezterm-gui needs #![allow(unexpected_cfgs)] (rust-objc issue 125), as window/src/os/macos/mod.rs does.
