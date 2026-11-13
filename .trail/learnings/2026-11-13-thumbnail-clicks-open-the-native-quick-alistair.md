---
title: "Thumbnail clicks open the native Quick Look panel, controlled by the window's view"
author: Alistair
date: 2026-11-13
branch: claude/multiline-paths-inline-images-0a925e
cost: { minutes: 3, tokens: 27125 }
id: l-4f315afd
scope: repo
status: active
src: agent
touches:
  - window/src/os/macos/window.rs
---

## Decided
- Clicking a thumbnail opens `QLPreviewPanel` from WezTerm's own process. The first version ran `qlmanage -p`, which showed "[DEBUG]" in the title, took the menu bar, and opened a new window per click.
- The WezTerm view answers `acceptsPreviewPanelControl:`, `beginPreviewPanelControl:` and `endPreviewPanelControl:`. The panel finds it through the key window's responder chain. The items are plain NSURLs, which already are preview items.

## Watch out
- Show the panel from a fresh main-thread task, not inside the mouse event: it takes the key window, which calls back into the window being handled.
- QLPreviewPanel is in Quartz.framework; without linking it the class isn't loaded.

In the code: window/src/os/macos/quick_look.rs, the three methods on the view class in window/src/os/macos/window.rs, open_quick_look in wezterm-gui/src/termwindow/image_preview.rs.
