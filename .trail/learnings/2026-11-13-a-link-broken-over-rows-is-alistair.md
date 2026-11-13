---
title: "A link broken over rows is joined on hover when the joined path exists, and drawn from a copy of the row"
author: Alistair
date: 2026-11-13
branch: claude/multiline-paths-inline-images-0a925e
cost: { minutes: 7, tokens: 76200 }
id: l-847055a2
scope: repo
status: active
replaces: l-beaad083
src: agent
---

## Decided
- Claude Code breaks long paths with real newlines plus an indent, so the hyperlink rules link each row alone. On hover the GUI finds the chain: a rule-made link that ends a row, then the first token of each next row (same indent, no further right than the link).
- The rows count as one link if the joined text is a file that exists, or every row but the last is filled to the pane's right edge. Existence matters most: Claude keeps a right margin, and rows printed before a resize stay broken at the old width, so broken rows usually stop short of the edge.
- Nothing is written to the pane's lines. The window remembers the joined link and its pieces; the renderer draws those rows from a copy that carries the link.

## Tried and ruled out
- Requiring the row to reach the pane's edge: failed in the user's real sessions for the reasons above.
- Writing the joined link onto the lines while hovered: a mux client's pane (the user's normal setup, `connect unix`) hands out copies, so it never showed there.

## Watch out
- The copy needs `clear_appdata()`, or it reuses the original row's cached shape hash.
- `start --always-new-process` gives a local pane even with the user's config; use `wezterm-gui connect unix` to test what they run.

In the code: wezterm-gui/src/termwindow/wrapped_link.rs (find_chain, chain_from, set_link), link_under_mouse and JoinedLink in mouseevent.rs, render_line in render/pane.rs.
