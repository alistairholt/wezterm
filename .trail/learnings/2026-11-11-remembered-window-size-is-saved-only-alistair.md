---
title: Remembered window size is saved only on real window resizes
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 4, tokens: 25504 }
id: l-e9e6a505
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/resize.rs
  - window/src/os/macos/save_panel.rs
---

## Decided
remember_window_size saves the size only when the window itself was resized, not when a window closes. With hide_tab_bar_if_only_one_tab, opening a second tab shows the tab bar and the terminal loses its rows while the window keeps its size. Saving on close then made each new window a row smaller than the last. The resize that ends a drag already saves the size, so the close-time save added nothing else.

## Tried and ruled out
Saving in TermWindow's Drop (the first version) caused the shrinking described above. A code review found it.

## Watch out
The save panel looks for keyWindow, then mainWindow, then the frontmost window. Right after the app comes to the front keyWindow can be nil. Without a window it falls back to runModal, which runs a nested event loop.

In the code: wezterm-gui/src/termwindow/resize.rs remember_window_size; window/src/os/macos/save_panel.rs.
