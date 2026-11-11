---
title: "Multi-line paste asks first only when the program hasn't turned on bracketed paste"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 3, tokens: 42407 }
id: l-d633ef26
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/clipboard.rs
  - mux/src/pane.rs
---

## Decided
confirm_multiline_paste (default on, like kitty) asks through the existing confirm overlay when the text has \n or \r and the pane's program hasn't set DECSET 2004. Pane gained bracketed_paste_enabled(), which defaults to true so remote/client panes, whose state we can't see, never prompt. Overlay panes (copy mode etc.) don't prompt either. A confirmed paste goes to the real pane and its Broadcast Input targets.

## Watch out
- Testing the confirm overlay by background click: computer-use clicks briefly focus the window, and swallow_mouse_click_on_window_focus (default true on macOS) eats that click. Set it to false in the test config, then a single click on [Y]es / [N]o works.
- The test needs the system clipboard; save it with pbpaste first and restore it with pbcopy after.

## In the code:
wezterm-gui/src/termwindow/clipboard.rs paste_with_confirmation; mux/src/pane.rs bracketed_paste_enabled (LocalPane reads Terminal::bracketed_paste_enabled).
