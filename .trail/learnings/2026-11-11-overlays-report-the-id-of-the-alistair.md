---
title: "Overlays report the id of the pane under them, so check pane_state overlay, not the id"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 1, tokens: 15989 }
id: l-7b214cd2
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/keyevent.rs
  - wezterm-gui/src/termwindow/mod.rs
---

## Watch out
CopyOverlay and QuickSelectOverlay return the underlying pane's id from pane_id(). A check like "is this pane id one of the tab's panes" passes for them. Broadcast Input did that, so keys typed in copy mode or Quick Select were copied to the other panes. Test `self.pane_state(pane.pane_id()).overlay.is_some()` instead.

## Decided
ShowScrollbackInPager spawns in the "local" domain, because its temp file only exists locally; in an SSH or remote mux pane the default CurrentPaneDomain ran the pager remotely, where it failed and left the scrollback file behind. A 30s timer also removes the file, because the spawn can fail (for example a cwd that no longer exists) and then nothing else would.

## In the code:
wezterm-gui/src/termwindow/keyevent.rs broadcast_input_targets; wezterm-gui/src/termwindow/mod.rs show_scrollback_in_pager. Both found by a review of the batch.
