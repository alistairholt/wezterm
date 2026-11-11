---
title: Mux server waits for redraw on unix-domain live resize
author: Alistair
date: 2026-11-11
branch: resize-wait-mux
cost: { minutes: 75, tokens: 396459 }
id: l-8aeb44b1
scope: repo
status: active
src: agent
touches:
  - "wezterm-mux-server-impl/src/sessionhandler.rs#resize_pane"
  - "wezterm-client/src/pane/clientpane.rs#PendingResizeReply"
  - "wezterm-client/src/client.rs#send_pdu_now"
  - "wezterm-gui/src/termwindow/resize.rs#LiveResizeRepaintWait"
---

Extends the local-pane wait (l-e6d7d99d) to ClientPane; its "ClientPane is not waited for" note no longer holds.

## Decided
- The mux server does the waiting. On a Resize with repaint_wait_ms > 0 (codec 47), resize_pane takes the pane's PaneOutputSignal mark before resizing, waits on smol::unblock so the server's main thread keeps serving, then computes the changes on the main thread and returns them in ResizeRepaintResponse.
- While that wait is open the session holds back its own pushes for the pane (PerPane.resize_repaint_waits). Without this the parser's quiet-flush PaneOutput push usually beats the reply, so the reply carries nothing and the redraw lands in a main-thread task the GUI only runs after it has painted.
- The client sends every Resize with Client::send_pdu_now (try_send, no spawned task). The reply goes from the client thread straight into a channel, so the GUI main thread can block on it with a timeout and apply it itself; nothing it waits for needs the main thread. A reply nobody waited for is applied from PendingResizeReply's Drop.
- Never drop the response Receiver of a sent PDU before the reply comes: client_thread_async treats a failed try_send as ClientWasDestroyed and drops the connection.
- Updates queued on the main thread before a waited-for reply are older than it and are skipped by seqno (RenderableInner.resize_reply_seqno); Terminal::resize bumps seqno, so pre-resize pushes are always strictly older.
- Only unix domains (Client.is_local) arm the wait; over a network the round trip alone would eat the budget each step.

## Watch out
- REPAINT_NO_OUTPUT (1.5ms) stops the wait before a program that starts writing later than that after SIGWINCH; a test program that slept 2ms got no-output on every step.
- Release build, unix socket, small Python TUI: Resize round trip about 4ms with the wait (server waited about 2.2ms) vs 0.45ms without.
