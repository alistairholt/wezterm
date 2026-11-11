---
title: WebGpu vertex buffers live in shared Metal memory and are written in place
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 21, tokens: 323533 }
id: l-1d81fd20
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/renderstate.rs
  - wezterm-gui/src/termwindow/webgpu.rs
  - wezterm-gui/src/termwindow/render/draw.rs
---

## Decided
Vertex buffers on the WebGpu path are MTLBuffers in shared storage, made once per capacity and wrapped for wgpu with buffer_from_raw + create_buffer_from_hal. Quads are written straight into them; before writing, a buffer waits (device.poll WaitForSubmissionIndex) for the frame that last drew from it. TripleVertexBuffer rotates three, so that frame is three frames old and the wait is almost always already satisfied. Measured on a 200x60 window at 60 fps: the draw step went from 49-62 ms/s of main-thread CPU to 11-13 ms/s, and presenter paint_ms from about 0.9-1.0 ms to 0.15 ms per frame. All layers now draw in one render pass with one uniform buffer, and the atlas bind groups are cached until the atlas texture changes.

## Tried and ruled out
- queue.write_buffer from a CPU-side Vec: removed the zero-fill but added a memmove of the same size into wgpu's staging buffer, so it saved almost nothing (0.6 ms/frame on a full redraw).
- The original per-frame create_buffer with mapped_at_creation: wgpu-core zero-fills a full-capacity staging buffer and copies all of it each frame.

## Watch out
- wgpu treats a buffer from hal as fully initialized (BufferInitTracker::new(0)), so it never clears it; that is what makes CPU writes safe to draw from.
- Poll at most once per frame: wait_for_frame remembers the newest finished frame. Polling per buffer cost about 0.1 ms/frame in IOKit status checks.
- A frame that is skipped (no drawable) never marks its buffers, so they are not waited on.

In the code:
- wezterm-gui/src/renderstate.rs WebGpuVertexBuffer (vertices_mut, draw_slice)
- wezterm-gui/src/termwindow/webgpu.rs note_submission, wait_for_frame, atlas_bind_groups, write_uniform
- wezterm-gui/src/termwindow/render/draw.rs call_draw_webgpu
