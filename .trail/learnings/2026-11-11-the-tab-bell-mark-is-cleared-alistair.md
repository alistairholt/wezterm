---
title: "The tab bell mark is cleared while building tab information, not by a tab-switch hook"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 3, tokens: 129058 }
id: l-cacf7123
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - wezterm-gui/src/termwindow/mod.rs
  - wezterm-gui/src/tabbar.rs
---

## Decided
The "bell rang here" flag for kitty's bell_on_tab lives in the GUI's per-tab state, and it is cleared inside get_tab_information when that tab is active and the window is focused. Tab switches, focus changes and the bell itself all end in update_title, which rebuilds tab information, so one place covers every way of looking at the tab (click, key binding, cli activate-tab, focusing the window).

## Watch out
- The bell handler must call update_title after setting the flag, or the mark only appears at the next unrelated tab bar refresh.
- A bell in the active tab of an unfocused window is marked too, and clears when the window gains focus.
- Custom format-tab-title handlers don't get the prefix; they read TabInformation.has_unseen_bell.

## In the code:
wezterm-gui/src/termwindow/mod.rs: TabState.bell_pending, the Alert::Bell handler, get_tab_information. wezterm-gui/src/tabbar.rs compute_tab_title adds config.bell_on_tab. Tested with `wezterm cli spawn -- sh -c 'sleep 4; printf "\a"; exec sleep 600'` then `wezterm cli activate-tab --tab-index 0` (set WEZTERM_UNIX_SOCKET to gui-sock-PID and WEZTERM_PANE=0).
