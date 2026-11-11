---
title: "A mux client must keep its own tab size when it re-syncs the layout from the server, or old sizes echo back during a drag"
author: Alistair
date: 2026-11-11
branch: claude/mux-resize
cost: { minutes: 31, tokens: 151554 }
id: l-96416778
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - docs/changelog.md
  - docs/config/lua/config/macos_colorspace.md
  - wezterm-gui/src/termwindow/metal_present.rs
  - wezterm-gui/src/termwindow/mod.rs
  - wezterm-gui/src/termwindow/webgpu.rs
  - wezterm-mux-server-impl/src/sessionhandler.rs
  - wezterm-client/src/domain.rs
  - wezterm-mux-server-impl/src/dispatch.rs
---

## Decided
- ClientDomain::process_pane_list lays out a tab it already has at the tab's local size, not the server's root size. Every server-side resize sends TabResized, which makes the client re-sync; Tab::sync_with_pane_tree then calls tab.resize(size), and with the server's (older) size that resized the ClientPanes back and sent that old size to the server. On a fast drag (4 ms steps) the server ended at 87-107 cols instead of 122, with ~1000 resize calls per drag. With the local size: 122 every run, ~115 resizes.
- The session loop in dispatch.rs polls the client socket before the queued items. smol::future::or polls its first future first, and with a flooding pane there is always a push queued, so client requests (Resize, cli list) were never read; a release server stopped answering for minutes.
- SessionHandler::schedule_pane_push queues at most one push task per pane (PerPane.push_scheduled, cleared when the task starts). One task per PaneOutput made the GUI's own mux server (the one `wezterm cli` talks to) hang under a flood: cli list timed out at 15 s; now 0.2 s.

## Tried and ruled out
- Skipping server-side resizes superseded by a newer queued one: with the fixes above it never triggered (0 of ~110 per drag), so it was removed.
- A push straight after a non-waited Resize (kata 3t08): main already shows the new wrap within 0.05 s of a single resize, because the GUI repaints and polls; no gain, removed.

## Watch out
- GUI memory grows ~11 MB/s with a `yes` flood in a background mux tab, on main too. Separate bug.
- Don't turn on wezterm_client::pane=trace under a flood: it wrote a 6 GB log in a minute.
- The test: release mux server on its own unix socket (/private/tmp/claude-501/wzt/sock, short path), `yes` in a background tab, WEZTERM_DEBUG_RESIZE="4000;60,4,5,0,500", then compare `cli list` sizes from the server socket and the GUI socket 3 s later. Wrap cli calls in `perl -e "alarm 15; exec @ARGV"` so a starved server doesn't hang the script.

In the code: wezterm-client/src/domain.rs process_pane_list (layout_size); wezterm-mux-server-impl/src/dispatch.rs process_async; wezterm-mux-server-impl/src/sessionhandler.rs schedule_pane_push, mark_push_scheduled.
