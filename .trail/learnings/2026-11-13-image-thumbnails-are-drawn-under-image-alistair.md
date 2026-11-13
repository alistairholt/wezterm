---
title: "Image thumbnails are drawn under image paths by moving where rows are drawn, not by changing the terminal's rows"
author: Alistair
date: 2026-11-13
branch: claude/multiline-paths-inline-images-0a925e
cost: { minutes: 16, tokens: 140646 }
id: l-f857d355
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - wezterm-gui/src/termwindow/image_preview.rs
---

## Decided
- The user wanted thumbnails in the output itself, not a hover popup. The GUI gives each thumbnail rows of its own: a per-pane layout maps each viewport row to the row it is drawn at. If the text reaches the bottom, everything moves up instead so the last used row (cursor or last text) stays visible.
- The mouse goes through the same layout in reverse, in one place (mouse_event_terminal), so hover, clicks and selection still hit the right row. A thumbnail is a UI item; clicking it runs `qlmanage -p` with every image on screen, the clicked one first.
- Only tokens with an image extension and a `/` are candidates. Bare names were dropped so `ls` in a photo folder doesn't fill the pane. Files are read and scaled on their own threads, four at a time.
- Off on the alternate screen: a full-screen program lays out every row itself.

## Tried and ruled out
- Putting image cells into the terminal's lines: the program (Claude Code) owns and redraws those rows.

## Watch out
- Anything drawn at `row * cell_height` in paint_pane must use `layout.visual_row` (lines, cursor guide, command marks, FindCursor do). The IME position does not yet.
- The layout is cached per pane on (pane seqno, viewport, size, cursor row, cache generation). A file that changes on disk is only looked at again when that key changes.
- qlmanage shows "[DEBUG]" in its title and takes the menu bar; a native QLPreviewPanel would not.

In the code: wezterm-gui/src/termwindow/image_preview.rs (find_candidates, PreviewCache, PreviewLayout, image_preview_layout, paint_image_previews), paint_pane in render/pane.rs, mouse_event_image_preview in mouseevent.rs.
