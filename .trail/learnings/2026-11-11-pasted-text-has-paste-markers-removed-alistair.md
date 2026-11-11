---
title: "Pasted text has paste markers removed in the mux, not the terminal"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 14583 }
id: l-27a1784b
scope: repo
status: active
src: agent
touches:
  - mux/src/localpane.rs
  - term/src/terminalstate/mod.rs
---

## Decided
The terminal's send_paste removes bracketed paste markers in one pass, so "ESC[201" + "ESC[201~" + "~" turns back into an end marker and the rest of the paste runs as typed input. The term crate is kept unchanged in this fork, so LocalPane::send_paste removes the markers until none are left before calling it. Every paste reaches a local pane through there: clipboard, drag and drop, quick select, wezterm cli send-text and the mux server.

## Watch out
A fix in wezterm-gui's clipboard code alone would miss drag and drop, quick select and cli send-text.
To test it end to end, run a pane that turns on bracketed paste (printf '\033[?2004h'), sets stty raw -echo and dumps its input with head -c N, then paste with wezterm cli send-text.

In the code: strip_paste_brackets in mux/src/localpane.rs; the single pass is in TerminalState::send_paste in term/src/terminalstate/mod.rs.
