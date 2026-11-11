---
title: "AppKit sheets must be shown from a separate task, not inside a key action"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 4, tokens: 38327 }
id: l-37962629
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
SaveScrollbackToFile (iTerm2's Save Contents) shows an NSSavePanel as a sheet with beginSheetModalForWindow:completionHandler:, from a task started with promise::spawn::spawn. Key actions run inside the window crate's with_window_inner, which holds a borrow of the window's Inner. Attaching a sheet calls the window delegate straight away, which borrows Inner again and panics ("already borrowed" at window.rs did_resign_key and nearby). A task spawned from the action runs later, outside that borrow. The sheet, not runModal, is used so no nested event loop runs. The completion block gets the path.

## Tried and ruled out
Calling beginSheetModalForWindow straight from perform_key_assignment crashed the GUI at once.

## Watch out
block 0.1 ConcreteBlock needs an Fn closure. Keep the FnOnce callback in a RefCell<Option<_>> and take it.
The panel defaults to ~/Documents. In tests, click Save through System Events ("click button \"Save\" of splitter group 1 of sheet 1 of window 1"), then delete the file.
A crash leaves a "quit unexpectedly" dialog owned by UserNotificationCenter on top of later test windows. Click its Ignore button through System Events.

In the code: window/src/os/macos/save_panel.rs choose_save_path; wezterm-gui/src/termwindow/mod.rs save_scrollback_to_file.
