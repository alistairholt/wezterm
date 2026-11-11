---
title: Metal presenting in step with macOS live resize
author: Alistair
date: 2026-11-10
branch: metal-design
cost: { minutes: 10, tokens: 44744 }
id: l-9853f2b0
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/webgpu.rs
  - window/src/os/macos/window.rs
  - wezterm-gui/src/termwindow/render/draw.rs
---

## Decided
To present each live-resize frame in the same Core Animation transaction as the window change, WezTerm must own the CAMetalLayer drawable instead of using wgpu::Surface. wgpu-hal 25 sets presentsWithTransaction from a plain bool on hal::metal::Surface in every configure (wgpu-hal-25.0.2/src/metal/surface.rs:162), Surface::as_hal only gives &Surface, and WebGpuState::resize calls configure on every size step, so setting the flag on the layer yourself gets overwritten. Plan: docs/internal/metal-renderer-plan.md.

## Tried and ruled out
- Setting presentsWithTransaction on the layer while keeping wgpu::Surface: reset by configure on each resize step.

## Watch out
- Proven headless (docs/internal/metal-spike): set drawableSize, nextDrawable, wrap with wgpu::hal::metal::Device::texture_from_raw + Device::create_texture_from_hal, render with wgpu, submit, then an empty command buffer on wgpu's raw MTLCommandQueue (Queue::as_hal -> as_raw) + waitUntilScheduled, then drawable.present inside the transaction. 40/40 steps exact size.
- nextDrawable took ~6.5 ms on a detached (not composited) layer; measure on screen before trusting any budget.
- The backing layer's contentsScale is forced to 1.0 and only looks right because contentsGravity defaults to resize; a non-scaling gravity needs contentsScale = backingScaleFactor.
