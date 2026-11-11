---
title: "Moving the active pane out uses the mux's move_pane_to_new_tab"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 18946 }
id: l-ecd43984
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
MovePaneToNewTab and MovePaneToNewWindow (kitty's detach_window, iTerm2's Move Session) call Mux::move_pane_to_new_tab, which PaneSelect's MoveToNewTab and MoveToNewWindow modes and the Lua pane:move_to_new_tab already use. They run it from a spawned task, because it is async and goes through the pane's domain. The pane is taken with get_active_pane_no_overlay, so an open overlay isn't what moves. A pane alone in its tab doesn't move to a new tab, since it already has one. For a new window, its whole tab moves with move_tab_to_window, unless that is the window's only tab.

## Watch out
After the move the focus goes to the moved pane's tab. A test that splits again must first activate a pane in the old tab (wezterm cli activate-pane).

In the code: wezterm-gui/src/termwindow/mod.rs move_active_pane_out.
