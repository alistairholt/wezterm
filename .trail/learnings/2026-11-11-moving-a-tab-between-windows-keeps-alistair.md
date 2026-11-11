---
title: "Moving a tab between windows keeps the new window's builder alive until the tab is in it"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 3, tokens: 31741 }
id: l-de357fac
scope: repo
status: active
src: agent
touches:
  - mux/src/lib.rs
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
Mux::move_tab_to_window moves an existing Tab between windows (or into a new window in the same workspace) instead of moving panes one by one, so splits and zoom survive. MoveTabToNewWindow and MergeAllWindows in the Window menu use it.

## Watch out
- Hold the MuxWindowBuilder from new_empty_window until add_tab_to_window has run, otherwise the GUI is told about an empty window and closes it. move_pane_to_new_tab does the same.
- Send WindowInvalidated for the source window and call prune_dead_windows afterwards, so the old window's tab bar refreshes and an emptied window closes.
- Tabs from remote (client) domains are moved only in the local mux; the server isn't told.
- Test without keypresses: count bells with wezterm.GLOBAL in the bell event to run different actions, ring with `printf '\a' | wezterm cli send-text --no-paste`, and check `wezterm cli list`.

## In the code:
mux/src/lib.rs move_tab_to_window; wezterm-gui/src/termwindow/mod.rs MoveTabToNewWindow and MergeAllWindows in perform_key_assignment.
