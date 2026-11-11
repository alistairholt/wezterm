---
title: "CopyOrInterrupt sends Ctrl-C through the pane's key_down"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 3, tokens: 28862 }
id: l-07922a22
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - docs/changelog.md
  - docs/config/lua/config/confirm_large_paste_bytes.md
  - wezterm-gui/src/termwindow/clipboard.rs
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
CopyOrInterrupt (kitty's copy_or_interrupt) sends Ctrl-C with pane.key_down(KeyCode::Char('c'), CTRL), not by writing a 0x03 byte. key_down goes through the terminal's key encoding, so programs that turned on the kitty keyboard protocol or another key mode get the encoding they asked for. "No selection" means selection_text() is empty, the same text CopyTo would copy.

## Watch out
In termwindow/mod.rs, KeyCode and Modifiers are the window crate's types. Pane::key_down needs wezterm_term::KeyCode and wezterm_term::KeyModifiers, so write the full paths.
To test the interrupt without keystrokes, run a bash script that traps INT and echoes, and bind the action to the bell event.

In the code: the CopyOrInterrupt arm of perform_key_assignment in wezterm-gui/src/termwindow/mod.rs.
