---
title: "Undo Close Tab keeps the closed tab out of every window, not in a hidden one"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 12, tokens: 159098 }
id: l-cc9a989b
scope: repo
status: active
src: agent
touches:
  - config/src/keyassignment.rs
  - docs/changelog.md
  - docs/config/default-keys.md
  - docs/config/lua/keyassignment/FindCursor.md
  - wezterm-gui/src/commands.rs
  - wezterm-gui/src/termwindow/mod.rs
  - wezterm-gui/src/termwindow/render/mod.rs
  - wezterm-gui/src/termwindow/render/pane.rs
  - docs/config/lua/keyassignment/CopyLastCommandOutput.md
  - docs/config/lua/keyassignment/SelectLastCommandOutput.md
---

## Decided
A tab closed with undo is taken out of its window but stays in the mux's tab list, so its panes keep running and nothing lists it (cli list, the tab bar and the workspace switcher all walk windows). After undo_close_tab_seconds a timer removes it for good. UndoCloseTab puts the newest live one back at its old index.

## Tried and ruled out
Moving the tab into a window in a hidden workspace: that window would show up in workspace lists and launcher menus, and could keep the app from quitting.

## Watch out
- The last tab in a window is never kept for undo. Mux::is_empty counts panes, so a kept tab would stop the app from closing when the last window goes.
- If a kept tab's programs exit, prune_dead_windows removes it; undo skips tabs the mux no longer has.
- In test configs, `wezterm.GLOBAL.n = 0` at the top level resets on every config evaluation; use `wezterm.GLOBAL.n = wezterm.GLOBAL.n or 0` for counters in event handlers.

In the code: mux/src/lib.rs detach_tab_from_window and restore_tab_to_window; wezterm-gui/src/termwindow/mod.rs close_tab_with_undo, undo_close_tab and CLOSED_TABS; overlay/confirm_close_pane.rs confirm_close_tab calls close_tab_with_undo.
