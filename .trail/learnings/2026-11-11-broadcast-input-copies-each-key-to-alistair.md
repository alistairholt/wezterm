---
title: "Broadcast input copies each key to the tab's other panes, encoded per pane"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 3, tokens: 37327 }
id: l-21e71bf1
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/keyevent.rs
---

## Decided
ToggleBroadcastInput is a per-tab flag in the GUI's TabState. Key presses go through one helper (send_key_to_pane) that picks win32, kitty or plain encoding for each target pane, because panes in one tab can ask for different keyboard protocols. Pastes, SendString, SendKey, composed (IME) text and lone modifier reports are copied too. Panes showing an overlay are skipped, and nothing is broadcast while the active pane is an overlay.

## Watch out
- Any new place that sends input to the active pane must also loop over broadcast_input_targets, or broadcast misses it.
- To test without keypresses: bind `act.Multiple { act.ToggleBroadcastInput, act.SendString '...' }` to the bell event, run `cat` in split panes, ring a bell with `printf '\a' | wezterm cli send-text --no-paste` (cat echoes it back), then compare `wezterm cli get-text --pane-id N`.

## In the code:
wezterm-gui/src/termwindow/keyevent.rs broadcast_input_targets, send_key_to_pane; perform_key_assignment SendString/SendKey/ToggleBroadcastInput in termwindow/mod.rs; clipboard.rs paste.
