---
title: AppKit treats command line arguments as files to open unless told not to
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 9, tokens: 90824 }
id: l-82e2ea55
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - docs/changelog.md
  - docs/config/lua/config/notify_on_command_finish.md
  - wezterm-gui/src/termwindow/mod.rs
  - window/src/os/macos/connection.rs
  - window/src/os/macos/app.rs
  - window/src/os/macos/window.rs
---

## Decided
AppKit reads command line arguments it doesn't recognise as files to open, so `wezterm-gui --config-file X start` calls application:openFile: with "start". Once opens that arrive before launch finished were queued instead of dropped, every launch from a shell opened a second window that ran "start" as a script and closed about 0.4s later. That stray window is also why mux window, tab and pane ID 1 were skipped at startup. The fix registers NSTreatUnknownArgumentsAsOpen = NO through registerDefaults before NSApp starts. registerDefaults is used, not setObject:forKey:, because it is not saved to the app's preferences. Files opened from Finder or `open` arrive as Apple Events, so they still work.

## Tried and ruled out
Dropping pre-launch opens again was rejected: double-clicking a script with WezTerm closed needs them.

## Watch out
If IDs start at 1 again, or a window flashes at startup, run with WEZTERM_LOG=window::os::macos::app=debug and look for "application_open_file".
Window sizes in pixels must be divided by the backing scale as floats. With integer division, an odd row count with a 35 px cell on a 2x screen loses a row.

In the code: window/src/os/macos/connection.rs Connection::create_new; window/src/os/macos/app.rs application_open_file; window/src/os/macos/window.rs new_window size.
