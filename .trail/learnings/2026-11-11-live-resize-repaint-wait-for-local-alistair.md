---
title: Live-resize repaint wait for local panes
author: Alistair
date: 2026-11-11
branch: resize-wait
cost: { minutes: 42, tokens: 328585 }
id: l-e6d7d99d
scope: repo
status: active
src: agent
touches:
  - "mux/src/lib.rs#parse_buffered_data"
  - "wezterm-gui/src/termwindow/resize.rs#LiveResizeRepaintWait"
---

## Decided
- The wait signal comes from the pane's parser thread (parse_buffered_data), through mux::output_signal::PaneOutputSignal (a parking_lot Mutex + Condvar per local pane, kept in Mux.output_signals). It reports raw reads, whether parsed actions are still unapplied (coalescing or an open DEC 2026 block), and sync-block ends. Pane seqno and MuxNotification::PaneOutput were not used: PaneOutput is delivered on the main thread, which is the thread doing the waiting, and seqno only moves after the coalesce flush, so it can't tell "no output yet" from "output still being coalesced".
- send_actions_to_mux applies actions to the Terminal on the parser thread, not the main thread, so blocking the main thread for a few ms does not stop the model from updating.
- While a RepaintWait exists, the parser flushes after 1ms of quiet instead of mux_output_parser_coalesce_delay_ms (3ms) from the first read. Otherwise the first flush lands ~3.4ms after SIGWINCH and a redraw longer than 3ms gets split over two flushes.

## Watch out
- ClientPane (remote mux) is not waited for: its Resize PDU is sent from a main-thread task and render deltas are applied on main-thread tasks (RefCell state, plus hydrate_lines round trips), so blocking the main thread would just burn the budget. Doing it needs the resize PDU sent synchronously, render-delta apply moved off the main thread, and the server to push the delta after its own quiet/sync-end signal.
- RepaintWait's Drop decrements the waiter count; leaking one would leave that pane's parser in quick-flush mode (harmless but changes coalescing).
