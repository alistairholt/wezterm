---
title: "Check the Dock badge through the Dock's accessibility tree, with the test build wrapped in a temporary app bundle"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 4, tokens: 40496 }
id: l-185bc03e
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - docs/changelog.md
  - docs/config/lua/TabInformation.md
  - docs/config/lua/config/bell_on_tab.md
  - wezterm-gui/src/tabbar.rs
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
macos_dock_badge_on_bell sums unseen-bell tabs over all GUI windows in a main-thread thread_local and only calls setBadgeLabel when the total changes, because update_title runs often.

## Tried and ruled out
- Reading the badge from `lsappinfo info -only StatusLabel`: it prints nothing useful for an unbundled process.
- Reading the Dock's "WezTerm" item while running target/release/wezterm-gui: that item is the installed /Applications/WezTerm.app. The unbundled test binary has no Dock item, so its badge is never visible.

## Watch out
Wrap the test binary in a throwaway bundle with its own CFBundleIdentifier and name (copy wezterm-gui and wezterm into X.app/Contents/MacOS, minimal Info.plist), start it with `open -n X.app --args ...`, then read `value of attribute "AXStatusLabel" of UI element "<name>" of list 1` from System Events' "Dock" process. A minimal Swift app setting dockTile.badgeLabel is a good control for the method.

## In the code:
window/src/os/macos/dock.rs set_dock_badge; wezterm-gui/src/termwindow/mod.rs set_unseen_bell_count (called from update_title_impl and Drop).
