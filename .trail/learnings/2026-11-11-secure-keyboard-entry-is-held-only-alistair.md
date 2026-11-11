---
title: Secure Keyboard Entry is held only while WezTerm is the active app
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 4, tokens: 44447 }
id: l-37007d70
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - docs/changelog.md
  - docs/config/lua/config/audible_bell.md
  - docs/config/lua/config/window_alert_on_bell.md
  - wezterm-gui/src/termwindow/mod.rs
  - window/src/lib.rs
  - window/src/os/macos/window.rs
  - window/src/os/macos/app.rs
  - wezterm-gui/src/commands.rs
---

## Decided
Secure input (Carbon EnableSecureEventInput) applies to the whole login session, so WezTerm enables it only while it is the active app, like iTerm2. The app delegate's applicationDidBecomeActive/applicationDidResignActive call secure_input::sync(), which keeps Enable/Disable calls balanced (macOS counts them).
`macos_secure_keyboard_entry` is applied only when its value changes, so a config reload doesn't undo a toggle from the menu. The menu checkmark is set with NSMenuItem setState, both when the menu bar is rebuilt and after a toggle.

## Watch out
To test activation, `tell application "Finder" to activate` from osascript often does nothing on recent macOS; use System Events `set frontmost of process ...`. Check the owner with `ioreg -l -w 0 | grep kCGSSessionSecureInputPID`. Pressing the menu item through computer-use needs full tier on WezTerm, which this setup doesn't grant, so the toggle path was checked only by building.

In the code: window/src/os/macos/secure_input.rs; window/src/os/macos/app.rs; wezterm-gui/src/commands.rs apply_secure_keyboard_entry_config, update_secure_keyboard_entry_menu_state; KeyAssignment::ToggleSecureKeyboardEntry.
