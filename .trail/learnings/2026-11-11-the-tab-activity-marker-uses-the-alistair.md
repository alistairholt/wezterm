---
title: "The tab activity marker uses the panes' own unseen-output state, so test it with the window focused"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 23857 }
id: l-f8359ad5
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/mod.rs
  - wezterm-gui/src/tabbar.rs
---

## Decided
tab_activity_symbol (off by default, as in kitty) reads Pane::has_unseen_output for every pane in the tab when the tab bar is built. That needed no new hook, because the terminal already raises Alert::OutputSinceFocusLost once per focus loss and the GUI calls update_title for it. Bell wins over activity when both apply.

## Watch out
has_unseen_output is `!focused && seqno > lost_focus_seqno`. A pane that has never had focus counts as unseen however little it printed. So in a test window that was never focused, every background tab is marked. Bring the window to the front with osascript (`set frontmost of (first process whose unix id is PID) to true`) before spawning tabs with `wezterm cli spawn`.

## In the code:
wezterm-gui/src/termwindow/mod.rs get_tab_information (has_unseen_output); wezterm-gui/src/tabbar.rs compute_tab_title.
