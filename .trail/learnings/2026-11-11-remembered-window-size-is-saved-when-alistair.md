---
title: "Remembered window size is saved when a resize ends, not during a drag"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 1, tokens: 5351 }
id: l-59a5cf98
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - window/src/os/macos/connection.rs
  - window/src/os/macos/window.rs
  - wezterm-gui/src/termwindow/resize.rs
---

## Decided
remember_window_size (like kitty's option) saves the window's size in cells, not pixels, so a font size change doesn't distort it. The size goes into DATA_DIR/window-size, and initial_size() reads it, so every spawn path that opens a window picks it up without changes. The save happens on the resize that ends a live resize, any other non-live resize, and when a window closes. During a drag, resizes are live and AppKit sends one per size step, so saving then would write the file dozens of times a second. ResetFontAndWindowSize uses configured_initial_size(), so it still goes back to initial_cols/initial_rows.

## Tried and ruled out
Saving the size of a maximized or full screen window was ruled out: new windows would open screen-sized without being in that state.

## Watch out
The resize that ends a live resize usually repeats the last step's size, and resize() returns early on "dimensions didn't change". The save must also run on that early return, or a drag is never saved.
OpenCommandScript (scripts and folders opened from Finder) used TerminalSize::default(), always 80x24. It now uses initial_size().

In the code: config/src/window_size.rs; Config::initial_size and configured_initial_size; wezterm-gui/src/termwindow/resize.rs remember_window_size.
