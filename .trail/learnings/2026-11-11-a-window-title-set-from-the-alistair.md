---
title: A window title set from the cli or Lua must be kept apart from pane escape-sequence titles
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 4, tokens: 98447 }
id: l-0b1aa585
scope: repo
status: active
src: agent
touches:
  - mux/src/window.rs
  - mux/src/localpane.rs
  - wezterm-client/src/domain.rs
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
The mux window title is written both by `set_title` (cli `set-window-title`, Lua `MuxWindow:set_title`) and by every pane's OSC 2. Showing the mux title as-is would let a background tab's prompt hijack the window title, so Window now records whether the latest title was explicit. The GUI's default title uses it only when explicit; a later OSC 2 from any pane hands control back to the active pane's title.

## Watch out
Titles arriving from a remote mux server are treated as pane titles (the PDU carries no flag), so a cli title set on a remote server doesn't pin the client window title.

In the code: mux/src/window.rs set_title / set_title_from_pane / get_explicit_title; mux/src/localpane.rs LocalPaneNotifHandler; wezterm-client/src/domain.rs process_remote_window_title_change; wezterm-gui/src/termwindow/mod.rs update_title_impl. Upstream #4899.
