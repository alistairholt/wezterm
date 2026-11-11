---
title: "Bell bounces the Dock icon only when WezTerm is in the background, on by default like kitty"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 4, tokens: 41881 }
id: l-e8dfa381
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - wezterm-gui/src/termwindow/keyevent.rs
  - wezterm-input-types/src/lib.rs
  - window/src/os/macos/window.rs
  - wezterm-gui/src/termwindow/mod.rs
  - config/src/config.rs
---

## Decided
`window_alert_on_bell` (kitty's name) defaults to true and calls NSApp requestUserAttention with NSInformationalRequest (one bounce), only when the window isn't focused. AppKit ignores the request while the app is active anyway; the focus check also keeps it off for a background window of an active app.

## Watch out
You can't check the bounce from a screenshot on this machine: the Dock is auto-hidden. A wezterm-gui started from a shell doesn't take focus, so tests run "unfocused" unless you bring it forward with System Events `set frontmost of process <pid>`. The debug log line "bell in unfocused window: requesting attention" shows the call.

In the code: window/src/os/macos/window.rs WindowOps::request_attention; wezterm-gui/src/termwindow/mod.rs Alert::Bell handler; config/src/config.rs window_alert_on_bell.
