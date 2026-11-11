---
title: "Window transparency toggle goes through the window's config overrides"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 20695 }
id: l-7731a61f
scope: repo
status: active
src: agent
touches:
  - config/src/keyassignment.rs
  - docs/changelog.md
  - docs/config/lua/keyassignment/ToggleWindowOpacity.md
  - wezterm-gui/src/commands.rs
  - wezterm-gui/src/termwindow/mod.rs
  - Cargo.lock
  - Cargo.toml
  - docs/config/lua/keyassignment/SaveScrollbackToFile.md
  - window/Cargo.toml
  - window/src/os/macos/mod.rs
---

## Decided
ToggleWindowOpacity (iTerm2's Use Transparency) writes window_background_opacity = 1.0 into the window's config_overrides and calls config_was_reloaded(), the same path window:set_config_overrides takes. That avoids a second opacity setting the renderer would have to check everywhere it reads window_background_opacity. It only removes the key if it set it and the value is still 1.0, so a Lua set_config_overrides call that replaced the overrides is not undone.

## Watch out
A Lua set_config_overrides call replaces the whole override object, toggled key included. After one, the next toggle makes the window opaque again rather than translucent.

In the code: wezterm-gui/src/termwindow/mod.rs toggle_window_opacity, TermWindow.opacity_override_set.
