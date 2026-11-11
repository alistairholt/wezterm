---
title: "Test macOS live resize with the built-in resize driver, not synthetic mouse drags"
author: Alistair
date: 2026-11-10
branch: main
cost: { minutes: 23, tokens: 73700 }
id: l-9410b8f9
scope: repo
status: active
src: agent
touches:
  - window/src/os/macos/window.rs
---

## Decided
Live-resize testing on macOS uses a debug resize driver instead of synthetic mouse drags. Synthetic CGEvent drags moved the user's real pointer and misclicked into their other windows. The driver steps the window frame with setFrame:display:YES inside a simulated live resize, so it needs no mouse and runs while the user works, with exact sizes and timing on every run.

## Watch out
- The driver reproduces the stretching bug but about a third as often as a real drag (12-16 vs 35-38 squeezed frames per 60-step shrink on ProMotion). Use it to compare builds; confirm a fix with a few real drags.
- In the window crate, libc is not a macOS dependency (it sits under a non-mac target section), so `libc::` fails to build in the macOS backend. For host time use QuartzCore's CACurrentMediaTime via an extern block; it is also the clock motionrec's frames.csv uses.
- A probe one text row tall can't show vertical stretch from a one-step lag; use a probe many rows tall.

In the code:
- Branch resize-driver: window/src/os/macos/window.rs, mod debug_resize, WindowInner::debug_resize_step / debug_set_live_resizing
- Run: WEZTERM_DEBUG_RESIZE="4000;60,30,-5,0,500" (delay ms; then steps,interval_ms,dw,dh,hold_ms per segment). Logs "debug-resize ... host_time=" at info level.
