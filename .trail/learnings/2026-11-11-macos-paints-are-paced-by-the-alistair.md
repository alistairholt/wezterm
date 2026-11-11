---
title: "macOS paints are paced by the view's display link, paused while idle"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 16, tokens: 83910 }
id: l-e09f8460
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
Each window's view owns an NSView display link (macOS 14) added to the main run loop in common modes. invalidate() only marks the view invalidated and unpauses the link; each tick calls setNeedsDisplay if invalidated, else pauses the link. drawRect paints on every call, and the old max_fps timer is gone. max_fps becomes the link's preferredFrameRateRange (min(30, max_fps)..max_fps), updated on config reload.

Measured on the built-in ProMotion display, release build, max_fps = 120: a `yes` flood presents 120 frames/s instead of 60, and 50 MB of output finishes in 16 s instead of 23 s. Main-thread time per frame fell (scroll 3.7 -> 2.6 ms, redraw 3.2 -> 2.4 ms). Idle windows present nothing after startup. Hitches are unchanged (a few single-frame ones that also appear with the timer build at the same moments).

## Tried and ruled out
Keeping the timer: with a 1000/max_fps ms timer plus AppKit's display pass, paints topped out at about 60 frames/s even with max_fps = 120.

## Watch out
The link retains the view as its target and the run loop retains the link, so window_will_close invalidates it. A new code path that drops the view without going through windowWillClose: would leak the window.
The default max_fps is still 60, so ProMotion users only get 120 Hz if they raise it.
An unfocused window's cursor doesn't blink, so testing pause/resume needs real output (a script printing on a timer), not the cursor.

In the code: window/src/os/macos/window.rs: start_display_link, display_link_fired (selector weztermDisplayLinkFired:), WindowInner::invalidate, draw_rect, config_did_change, frame_rate_range and test frame_rate_range_follows_max_fps.
