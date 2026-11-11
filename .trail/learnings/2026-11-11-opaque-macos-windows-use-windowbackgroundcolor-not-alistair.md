---
title: "Opaque macOS windows use windowBackgroundColor, not clear"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 26388 }
id: l-736c9aed
scope: repo
status: active
src: agent
touches:
  - window/src/os/macos/window.rs
  - docs/changelog.md
---

## Decided
An opaque window's NSWindow background is NSColor.windowBackgroundColor instead of clearColor. On macOS 27 the native title bar draws over the window background, so a clear background made it transparent: in dark mode it captured as white with dark title text (upstream #8153; same fix as upstream PR #8240). The Metal layer covers the content area, so the colour only shows behind the title bar.

Checked on this machine (macOS 27, dark mode) by capturing the window's title bar before and after.

## Watch out
Translucent windows (window_background_opacity < 1) still use clear at alpha 0.01, the iTerm2 workaround for shadows. Whether their title bar has the same problem on macOS 27 wasn't checked.

In the code: window/src/os/macos/window.rs (window_background_color).
