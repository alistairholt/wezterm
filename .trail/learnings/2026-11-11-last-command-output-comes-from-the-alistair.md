---
title: "Last command output comes from the terminal's Output semantic zones"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 24878 }
id: l-2ca3bcb8
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - config/src/lib.rs
  - config/src/window_size.rs
  - docs/changelog.md
  - docs/config/lua/config/remember_window_size.md
  - wezterm-gui/src/frontend.rs
  - wezterm-gui/src/termwindow/mod.rs
  - wezterm-gui/src/termwindow/resize.rs
---

## Decided
ShowLastCommandOutputInPager (kitty's show_last_non_empty_command_output) reads pane.get_semantic_zones() and takes the newest Output zone that holds non-blank text. It shares the pager code with ShowScrollbackInPager (show_lines_in_pager). Output zones run from OSC 133;C to the next prompt mark, so the prompt and the command line are left out. term is not changed. Empty outputs are skipped, as kitty's non_empty variant does, because "the last command" is often one like `cd` that printed nothing.

## Watch out
To test without typing into WezTerm, use a default_prog script that prints the OSC 133 marks itself: A, `$ `, B, the command, C, the output, then "D;0". End it with a \a and bind the bell event to the action.

In the code: wezterm-gui/src/termwindow/mod.rs show_last_command_output_in_pager, show_lines_in_pager.
