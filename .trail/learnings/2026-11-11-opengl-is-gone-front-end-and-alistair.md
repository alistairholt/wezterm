---
title: OpenGL is gone; front_end and prefer_egl stay in config only so old configs load
author: Alistair
date: 2026-11-11
branch: claude/jolly-haslett-fc5244
cost: { minutes: 8, tokens: 186406 }
id: l-675fb5a9
scope: repo
status: active
src: agent
touches:
  - Cargo.lock
  - Cargo.toml
  - wezterm-gui/src/scripting/mod.rs
  - window/Cargo.toml
  - window/build.rs
  - window/examples/async.rs
  - window/src/bitmaps/mod.rs
  - window/src/lib.rs
---

## Decided
- The only renderer is wgpu's Metal backend through MetalPresenter. The glium/CGL/EGL path and the non-Metal wgpu backends were deleted, not kept behind a switch.
- front_end and prefer_egl still parse (the FrontEndSelection enum keeps OpenGL and Software) so existing configs load, but nothing reads them to pick a renderer. Don't remove the variants or the fields without a config migration story.
- RenderContext stayed as a thin wrapper over WebGpuState so GlyphCache and RenderState call sites didn't churn; the buffer enums collapsed to the WebGpu types.

## Watch out
- The macOS view always uses the window's backing scale for its layer now; the old 1.0-scale stretching setup was only for OpenGL.
- wgpu is built with default-features = false and only metal + wgsl. Anything that asks for another backend (Backends::all(), GL, Vulkan) finds no adapters.

In the code:
- wezterm-gui/src/renderstate.rs RenderContext; termwindow/render/draw.rs call_draw
- config/src/frontend.rs FrontEndSelection; config/src/config.rs front_end, prefer_egl
- Cargo.toml wgpu features; wezterm-gui/src/scripting/mod.rs enumerate_gpus uses Backends::METAL
