---
title: "The global hotkey uses Carbon RegisterEventHotKey, not an event monitor"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 3, tokens: 29918 }
id: l-50ece5f7
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/frontend.rs
  - config/src/config.rs
---

## Decided
macos_global_hotkey (iTerm2's hotkey) registers with Carbon's RegisterEventHotKey and an InstallEventHandler handler for kEventHotKeyPressed on the application event target. NSEvent addGlobalMonitorForEventsMatchingMask needs the Accessibility permission, can't consume the key, and doesn't see keys while the app is active. The key comes from the usual KeyNoAction config fields, resolved through key_map_preference, then PhysKeyCode, then the macOS virtual key code (keycodes.rs phys_to_vkey). It is registered at launch and again on every config reload, unregistering the old one first.

## Watch out
Testing it by pressing the hotkey means sending keystrokes, which isn't allowed in these sessions. Check the "registered global hotkey ... (vkey N)" debug log line, and test the toggle through the ToggleAppVisibility action bound to the bell event. Check the result with System Events `{visible, frontmost} of process`.
If RegisterEventHotKey fails, the usual cause is another app holding the same combination.

In the code: window/src/os/macos/global_hotkey.rs; wezterm-gui/src/frontend.rs apply_global_hotkey_config.
