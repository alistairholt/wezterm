---
title: Commands are timed even when notify_on_command_finish is off
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 4, tokens: 55786 }
id: l-63a736ea
scope: repo
status: active
src: agent
touches:
  - config/src/keyassignment.rs
  - docs/changelog.md
  - docs/config/lua/keyassignment/CopyLastCommandOutput.md
  - wezterm-gui/src/commands.rs
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
The pane's command start time is now recorded whatever notify_on_command_finish is set to. NotifyWhenCommandFinishes (iTerm2's Alert on Next Mark) has to work when the mode is Never, and it can be run while a command is already going. Recording a time and a string per WEZTERM_PROG change costs nothing that matters. The one-shot flag is taken only when a command that had a start time finishes, so a bare prompt redraw doesn't use it up.

## Watch out
Running the action twice cancels it: it toggles. It ignores notify_on_command_finish_after_secs.

In the code: wezterm-gui/src/termwindow/mod.rs track_command_for_notification and PaneState.notify_on_next_finish.
