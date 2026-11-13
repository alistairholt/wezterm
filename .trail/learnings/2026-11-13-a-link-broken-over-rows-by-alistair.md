---
title: "A link broken over rows by a program's own line breaks is joined only while the mouse is over it"
author: Alistair
date: 2026-11-13
branch: claude/multiline-paths-inline-images-0a925e
cost: { minutes: 12, tokens: 212073 }
id: l-beaad083
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/wrapped_link.rs
  - docs/changelog.md
---

## Decided
- Claude Code breaks long paths with real newlines plus an indent, so each row is its own logical line and the hyperlink rules link each piece alone. Soft-wrapped lines were never the problem; the rules already scan those as one line.
- On hover, the GUI looks for a chain: a rule-made link that ends a row filled to the right edge (2 cells of slack, row not soft-wrapped), then the first token of each next row, as long as those rows start no further right than the link and share one indent. The pieces get one new implicit link whose URI is the first link's URI plus the rest of the text.
- The joined link is temporary. It is taken off (and the rows handed back to the rules) as soon as the mouse is over anything else, or a row's seqno changes. Only one exists at a time.

## Tried and ruled out
- Joining at scan time and leaving the link on the lines: a row's joined link depends on its neighbor, and a line's implicit links are only cleared when that line itself changes, so stale links stay behind.
- Highlighting by Arc identity across the existing per-row links: the rest of a path often matches no rule ("y-description.png"), so there is no link on it to highlight. Underline is drawn per cell attribute, so the cells need a real link.

## Watch out
- Rows changed this way need `clear_appdata()` or the renderer keeps the cached quads (the shape hash is cached per line seqno).
- Mux client panes hand out copies of lines, so the link doesn't stay there; the code checks for that and falls back to the per-row link.
- Live test: CGEvent mouse moves reach an unfocused WezTerm window, clicks need it active. An `open-uri` handler that appends the URI to a file shows what a click opened.

In the code: wezterm-gui/src/termwindow/wrapped_link.rs (find_chain, apply_chain, remove_link), link_under_mouse and JoinedLink in termwindow/mouseevent.rs.
