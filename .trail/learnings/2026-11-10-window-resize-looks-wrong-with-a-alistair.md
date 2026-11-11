---
title: Window resize looks wrong with a mux domain because the client draws old lines until the server catches up
author: Alistair
date: 2026-11-10
branch: main
cost: { minutes: 13, tokens: 109321 }
id: l-cbe51e89
scope: repo
status: active
src: agent
touches:
  - wezterm-client/src/pane/clientpane.rs
  - wezterm-client/src/pane/renderable.rs
  - wezterm-mux-server-impl/src/sessionhandler.rs
  - window/src/os/macos/window.rs
---

## Decided
With a unix/SSH/TLS mux domain, the GUI never re-wraps text itself. On every resize step it marks all cached lines stale, sends a Resize to the server without waiting, and keeps drawing the old lines (wrapped at the old width) or empty lines until fresh ones arrive. The server handles each Resize on its main thread, behind pane output, and does a full re-wrap of scrollback for every intermediate size in a drag. Under heavy output the server falls seconds behind, so the user sees blank strips and text that hasn't re-wrapped.

Reproduced on macOS with a dev build: after a 22-step drag, an idle pane still showed the wrong wrap about 1.5 s after the drag ended. With another tab flooding output, the server's pane was still at 86 cols while the client had sized it to 100 cols, and the server stopped answering `wezterm cli list` for over 2 minutes (debug build, unbounded flood).

## Tried and ruled out
- In-flight line fetches from before a resize do NOT overwrite newer state: put_line only accepts a fetch result if the entry still carries that fetch's start time, and make_all_stale/Stale entries reject it (seen in the trace as "changed since fetch started ... leave it be").

## Watch out
- Stretching during live resize is a separate macOS drawing issue (draw_rect is capped at max_fps, and the OpenGL flushBuffer isn't synced with the window resize); it affects local panes too.
- To reproduce without touching a live mux: give the dev config its own unix_domains socket_path (macOS socket paths must be under ~104 chars, so the scratchpad is too long) and daemon_options pid_file, set no_serve_automatically, start target/debug/wezterm-mux-server yourself, and unset all WEZTERM_* env vars before launching.
- Synthetic edge drags with CGEvent only work when nobody else is using the mouse.

In the code:
- wezterm-client/src/pane/clientpane.rs ClientPane::resize (make_all_stale, detached Resize send)
- wezterm-client/src/pane/renderable.rs get_lines (stale -> old line, missing -> Line::with_width blank), put_line, poll backoff (BASE_POLL_INTERVAL 20ms doubling to MAX_POLL_INTERVAL 30s)
- wezterm-mux-server-impl/src/sessionhandler.rs Pdu::Resize -> spawn_into_main_thread
- window/src/os/macos/window.rs draw_rect paint_throttled
- Trace with WEZTERM_LOG='info,wezterm_client::pane=trace,wezterm_gui::termwindow::resize=trace'
