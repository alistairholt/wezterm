---
title: "A context menu must retain separator items, and must not open a second menu while one is tracking"
author: Alistair
date: 2026-11-11
branch: claude/context-menu
cost: { minutes: 7, tokens: 93503 }
id: l-b43d1e67
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - docs/changelog.md
  - docs/config/lua/config/command_status_marks.md
  - docs/shell-integration.md
  - mux/src/localpane.rs
  - mux/src/pane.rs
  - term/src/terminalstate/mod.rs
  - term/src/terminalstate/performer.rs
  - term/src/test/mod.rs
  - wezterm-gui/src/termwindow/command_marks.rs
---

## Decided
- ShowContextMenu (right click) builds a throwaway NSMenu whose items send weztermPerformKeyAssignment: with a nil target, so the chosen action reaches the key window the same way the menu bar does. It's shown from a spawned task, because popUp runs a nested event loop and the action dispatches back into the window.
- The menu opens at the click's position in the window's (flipped) content view, not at NSEvent.mouseLocation: a click delivered to a background window doesn't move the system pointer.
- color_scheme_light/dark are picked inside config's compute_extra_defaults from a global the GUI sets (config::set_system_dark_mode) before each reload. The appearance-change handler already reloaded the config, so no new hook was needed. The mux server never sets it, so it uses color_scheme.

## Watch out
- MenuItem::new_separator took separatorItem's autoreleased result as owned. The menu bar never frees its separators, so it was harmless there, but a discarded context menu over-released it and crashed in the autorelease pool pop after choosing an item. It now retains.
- computer-use sends a stray second click; with ShowContextMenu on Left Up in a test config, that opens a second menu after the first closes. A thread-local guard stops a menu opening inside another.
- While a menu is open, `wezterm cli` calls to the GUI hang until it closes.
- Menu items can be chosen with app_click at their position in the window frame (it does AXPress on the AXMenuItem).

In the code: window/src/os/macos/context_menu.rs, menu.rs new_separator, show_context_menu in wezterm-gui/src/termwindow/mod.rs, config::system_dark_mode.
