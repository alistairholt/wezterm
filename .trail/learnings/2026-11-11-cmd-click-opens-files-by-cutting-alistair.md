---
title: "Cmd-click opens files by cutting the text at the first colon followed by a digit, and is live-tested with a plain-click binding"
author: Alistair
date: 2026-11-11
branch: claude/open-file-path
cost: { minutes: 6, tokens: 163304 }
id: l-c3a31cd1
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/open_file.rs
---

## Decided
- OpenFileAtMouseCursor takes the text around the click up to whitespace, quotes, brackets, `,` or `;`, then cuts at the first `:` followed by a digit. Reading only a trailing `:N:N` missed grep's `file:12:match`.
- Paths with spaces are not supported: terminal output can't tell them from separators.
- A user's `open_file_command` runs through `$SHELL -l -i -c` (shlex-joined) so Homebrew tools are on PATH; the default `/usr/bin/open` runs directly.
- It only acts if the path exists, so a stray Cmd-click on a word does nothing.

## Watch out
- WezTerm drops mouse clicks (not wheel) sent to a window that isn't focused (`allow_action` in mouseevent.rs). Background clicks from computer-use do nothing until the test window is activated (NSRunningApplication.activate by pid).
- computer-use grants WezTerm the "click" tier, which blocks modifier-clicks, so a real Cmd-click can't be sent. Bind the action to a plain Left Up in a test config to exercise it.
- Lua reads `'\033'` as decimal 33 (`!`); use `'\27'` for ESC in test configs.

In the code: wezterm-gui/src/termwindow/open_file.rs (find_file_target, split_line_suffix), do_open_file_at_mouse_cursor in termwindow/mod.rs, SUPER Left Up default in inputmap.rs.
