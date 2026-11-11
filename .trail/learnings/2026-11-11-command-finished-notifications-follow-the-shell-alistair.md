---
title: "Command-finished notifications follow the shell integration's WEZTERM_PROG variable"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 3, tokens: 81966 }
id: l-bb8711ea
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
notify_on_command_finish (kitty's notify_on_cmd_finish) times commands from the WEZTERM_PROG user variable the shell integration already sets: it holds the command line while a command runs and is cleared at the next prompt. That avoids new escape sequences and keeps term unchanged. The timing lives per pane in the GUI's PaneState, because only the window that owns the pane knows if it is focused or the pane is visible.

## Tried and ruled out
OSC 133 semantic zones were not used for timing: they live in term and would need a hook there, and term is kept unchanged in this fork.

## Watch out
The toast fails from an unsigned target/release binary ("application must be code-signed for UNUserNotificationCenter"). Test the decision with WEZTERM_LOG=wezterm_gui::termwindow=debug, which logs "command finished ... notify=true/false". Put another app in front (osascript 'tell application "Finder" to activate') to test Unfocused.

In the code: wezterm-gui/src/termwindow/mod.rs track_command_for_notification, the SetUserVar arm of the mux notification handler; config NotifyOnCommandFinish.
