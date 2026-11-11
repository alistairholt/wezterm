---
title: Hyperlink rules scan a one-row line in place instead of cloning it
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 9, tokens: 93117 }
id: l-79588700
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/render/mod.rs
  - mux/src/pane.rs
  - wezterm-surface/src/line/line.rs
  - wezterm-surface/src/line/test.rs
  - docs/changelog.md
---

## Decided
When a logical line is a single physical line, applying hyperlink rules scans that line in place. The general path clones every physical line into one logical line, scans it, then splits it back, and the clone and free were most of the cost: about 23 of 32 ms/s on a full-screen redraw at 200x60, against about 9 ms/s for the regex scan. With the fast path the whole step fell from about 35 to 3.5 ms/s on redraw and from 15 to 4.5 ms/s while scrolling. Multi-row (wrapped) lines still use the clone path.

The fast path keeps the general path's behaviour: invalidate, scan, clear the line's appdata, and recompress the line if it was in cluster (scrollback) storage.

## Tried and ruled out
Doing it from the mux caller without touching wezterm-surface. The caller can't see whether a line was already scanned (the bit is private), and the appdata must be cleared exactly when a scan runs: the GUI caches each line's shape hash by seqno, and adding implicit links doesn't change the seqno. Clearing always would rehash every visible line every frame; clearing on "has a link" misses links added after a config reload.

## Watch out
This is a change to wezterm-surface, which this fork otherwise keeps as upstream.

In the code: wezterm-surface/src/line/line.rs, Line::apply_hyperlink_rules (the `if let [line] = logical_line` branch); test apply_hyperlink_rules_single_line in wezterm-surface/src/line/test.rs. Test with `cargo test -p wezterm-surface --features std,appdata,use_image`.
