---
title: Window resize with a mux domain shows old wrapping until the server catches up; test it against a release server
author: Alistair
date: 2026-11-10
branch: main
cost: { minutes: 36, tokens: 144186 }
id: l-3413a7ad
scope: repo
status: active
replaces: l-cbe51e89
src: agent
touches:
  - wezterm-client/src/pane/clientpane.rs
  - wezterm-client/src/pane/renderable.rs
  - wezterm-mux-server-impl/src/sessionhandler.rs
  - window/src/os/macos/window.rs
---

## Decided
With a unix/SSH/TLS mux domain the GUI never re-wraps text itself. Each resize step marks cached lines stale and sends a Resize without waiting; the GUI draws old-width (or blank) lines until the server sends re-wrapped ones. The macOS stretching during a drag is separate: draw_rect skips paints under the max_fps cap, and AppKit scales the stale GL frame. Skipping the cap while live_resizing cut squeezed frames from ~50 to ~0-3 per drag (120 fps recordings).

## Tried and ruled out
- Waiting for each Resize reply before sending the next (coalescing): under background output the reply can take seconds, so the final size never reached the server (stuck at 103 cols vs 137). Don't make resize delivery depend on reply latency.
- A local re-wrap preview in the client: no measurable gain once the server is a release build; base already settles in ~0.05-0.3 s.

## Watch out
- A debug mux server under an unbounded flood starves completely (no cli reply for 30 s+). That exaggerates the lag; measure with a release server.
- Don't share one CARGO_TARGET_DIR between git worktrees: cargo reused the main checkout's artifacts and the patched code was missing from the binaries. Check a patch-specific string with `strings` before measuring.
- Window recordings (motionrec) are sized at the start; when the window grows the video is scaled down, so normalise by a fixed-size element.
- The default renderer on macOS is CGL: prefer_egl defaults true but no libEGL ships, so EGL falls back.
- The server doesn't push after a Resize on an idle pane; the client only polls when the GUI paints.

In the code:
- wezterm-client/src/pane/clientpane.rs ClientPane::resize; wezterm-client/src/pane/renderable.rs get_lines/put_line/poll
- wezterm-mux-server-impl/src/sessionhandler.rs Pdu::Resize
- window/src/os/macos/window.rs draw_rect (paint_throttled vs live_resizing); config default_prefer_egl
