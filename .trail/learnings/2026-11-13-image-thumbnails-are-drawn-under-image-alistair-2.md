---
title: Image thumbnails are drawn under image paths by moving where rows are drawn; they must work on the alternate screen and for mux panes
author: Alistair
date: 2026-11-13
branch: claude/multiline-paths-inline-images-0a925e
cost: { minutes: 7, tokens: 76200 }
id: l-e615049a
scope: repo
status: active
replaces: l-f857d355
src: agent
---

## Decided
- Thumbnails get rows of their own: a per-pane layout maps each viewport row to where it is drawn, and the mouse goes through it in reverse. If the text reaches the bottom, rows move up instead.
- Claude Code runs on the alternate screen, so thumbnails are not turned off there. Several thumbnails share half the pane's rows (minimum three each).
- A token that ends a row is tried joined with the start of each following row; the reading that names an existing file wins. No "row reaches the edge" test.
- Candidates need an image extension and a `/`. Files load on their own threads, four at a time.

## Watch out
- A mux client's lines arrive after the first paint with no change to the pane's seqno. The layout also keeps a fingerprint of the line seqnos it was built from; paint compares it with the lines it drew and rebuilds on a mismatch.
- Anything drawn at `row * cell_height` in paint_pane must use `layout.visual_row`. The IME position does not yet.

In the code: wezterm-gui/src/termwindow/image_preview.rs, paint_pane in render/pane.rs, mouse_event_image_preview in mouseevent.rs.
