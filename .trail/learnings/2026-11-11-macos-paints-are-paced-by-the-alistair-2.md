---
title: "macOS paints are paced by the view's display link, paused while idle"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 4, tokens: 24750 }
id: l-fb096690
scope: repo
status: active
replaces: l-e09f8460
src: agent
touches:
  - docs/internal/metal-renderer-plan.md
  - window/src/os/macos/window.rs
  - config/src/config.rs
  - docs/config/lua/config/max_fps.md
  - docs/changelog.md
---

## Decided
Each window's view owns an NSView display link (macOS 14) on the main run loop. invalidate() marks the view and unpauses the link; each tick displays the view if marked, else pauses the link. drawRect paints on every call; the max_fps timer is gone. max_fps sets the link's frame rate range and now defaults to 120, since the link never runs faster than the display.

On the built-in ProMotion display a `yes` flood presents 120 frames/s instead of 60, 50 MB prints in 16 s instead of 23 s, and CPU per frame fell. Idle windows paint nothing after startup.

## Tried and ruled out
Keeping the timer: with max_fps = 120 it still topped out at about 60 frames/s.

## Watch out
The link retains the view and the run loop retains the link, so windowWillClose: invalidates it; any other path that drops the view would leak the window.
An unfocused window's cursor doesn't blink, so test pausing with output on a timer, not the cursor.

In the code: window/src/os/macos/window.rs (start_display_link, display_link_fired, invalidate, draw_rect, frame_rate_range); config/src/config.rs default_max_fps.
