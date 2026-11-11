---
title: "An untagged Metal layer already shows sRGB colors correctly, so sRGB as the default color space changes nothing you can see"
author: Alistair
date: 2026-11-11
branch: claude/macos-colorspace
cost: { minutes: 4, tokens: 43737 }
id: l-543d5572
scope: repo
status: active
src: agent
touches:
  - docs/config/lua/keyassignment/UndoCloseTab.md
  - wezterm-gui/src/frontend.rs
  - wezterm-gui/src/termwindow/mod.rs
  - docs/changelog.md
  - mux/src/lib.rs
  - mux/src/window.rs
  - config/src/config.rs
  - config/src/keyassignment.rs
  - docs/config/lua/config/undo_close_tab_seconds.md
  - wezterm-gui/src/commands.rs
---

## Decided
macos_colorspace defaults to "sRGB" and sets CAMetalLayer.colorspace explicitly. "DisplayP3" is there for people who want the more saturated look on a wide gamut display. "None" leaves the layer untagged.

## Tried and ruled out
The Metal plan (Task 7 / Q3) assumed a nil colorspace sends sRGB values to a P3 display unconverted, so colors looked oversaturated. Measured on macOS 27 with the built-in XDR display (Display P3), that's wrong: pure red #ff0000 captured as #ea3323 (sRGB red in P3) with both nil and sRGB, and as #ff0000 only with DisplayP3. So the explicit sRGB tag documents intent and protects against a different untagged default; it isn't a visible fix.

## Watch out
Measure with `screencapture -x -o -l <windowid>` and read the centre pixel with ffmpeg rawvideo. The PNG carries the display profile and the stored values are display-space, so they show what the window server composited. Don't use a config reload count from the log to check reloads; read the pixels before and after editing the config file.

In the code: set_layer_colorspace in wezterm-gui/src/termwindow/metal_present.rs; called from WebGpuState::new_impl and TermWindow::config_was_reloaded.
