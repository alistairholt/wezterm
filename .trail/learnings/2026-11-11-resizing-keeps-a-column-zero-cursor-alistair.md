---
title: Resizing keeps a column-zero cursor in place unless its own line was rewrapped
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 6, tokens: 124630 }
id: l-14aa35e2
scope: repo
status: active
src: agent
touches:
  - config/src/keyassignment.rs
  - docs/changelog.md
  - docs/config/lua/keyassignment/MovePaneToTab.md
  - wezterm-gui/src/commands.rs
  - wezterm-gui/src/termwindow/mod.rs
  - term/src/test/mod.rs
  - term/src/screen.rs
---

## Decided
The term crate is normally left unchanged in this fork, but the user approved this one-line fix (kata zd7m). The bug is platform-neutral and upstream has it too, so it is the kind of fix that should be shared both ways.

When the width changes, rewrap_lines moves a cursor that lands in column zero back to the end of the previous row. That should only happen when the cursor is on a later row of its own wrapped logical line. The old check (`adjusted_cursor.1 > 0`) was true for any row below the first, so a fresh prompt line after output jumped up to the end of the line above when the window got wider.

## Watch out
The term package is named `wezterm-term`: use `cargo nextest run -p wezterm-term`, not `-p term`.

In the code: term/src/screen.rs rewrap_lines checks `num_lines > 0`; test `test_resize_keeps_column_zero_cursor` in term/src/test/mod.rs. Live check: default_prog printing two lines then sleeping, widen 40 → 140 cols, `wezterm cli list` cursor stays (0,2); the old build gave (140,1).
