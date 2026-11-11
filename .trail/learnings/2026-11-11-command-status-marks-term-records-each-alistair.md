---
title: "Command status marks: term records each exit status against its prompt's row, and the GUI matches it to prompt zones"
author: Alistair
date: 2026-11-11
branch: claude/command-status-marks
cost: { minutes: 10, tokens: 128554 }
id: l-bc0ba9c7
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - config/src/keyassignment.rs
  - docs/changelog.md
  - docs/config/lua/config/open_file_command.md
  - docs/config/lua/keyassignment/OpenFileAtMouseCursor.md
  - docs/config/mouse.md
  - wezterm-gui/src/commands.rs
  - wezterm-gui/src/inputmap.rs
  - wezterm-gui/src/termwindow/mod.rs
  - wezterm-gui/src/termwindow/open_file.rs
---

## Decided
- The user approved a small change to term (normally kept unchanged): OSC 133;D used to be dropped. TerminalState now remembers the row where the last primary prompt started (133;A, N, or P;k=i, not k=s/r continuation or right prompts) and records the status against it in a BTreeMap, pruned to rows still in the scrollback. Doing this in term is exact; a GUI-only guess from a user variable could attach the status to the wrong prompt and only worked with WezTerm's own wezterm.sh.
- Pane::get_command_statuses has a default of no statuses, so remote mux panes get no marks. No codec change.
- The GUI only draws a mark where a prompt zone starts within 3 rows below the recorded row (prompts that begin with a blank line write no cells on that row) and before the next status. That hides stale statuses after a screen clear.
- Clicking a mark selects the output zones between the command line and the next finished command's prompt, not the next prompt zone, because continuation prompts are prompt zones too.

## Watch out
- computer-use app_click sends a second, stray press at negative coordinates. With default mouse bindings that press starts a new selection and clears the one the mark made. For live checks, set disable_default_mouse_bindings = true in the test config (this also keeps the clipboard untouched).
- Scripted CGEvent clicks are refused by the computer-use rules for click-tier apps, and the Claude app window usually covers the test window anyway.

In the code: term/src/terminalstate/mod.rs (note_prompt_start, note_command_status, command_statuses), wezterm-gui/src/termwindow/command_marks.rs, paint_command_marks in render/pane.rs, mouse_event_command_mark in mouseevent.rs.
