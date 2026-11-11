---
title: "Files opened before macOS finishes launching WezTerm are queued, and open beside the startup window"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 19818 }
id: l-787b463d
scope: repo
status: active
src: agent
touches:
  - window/src/os/macos/app.rs
---

## Decided
application:openFile: arrives before applicationDidFinishLaunching on a cold start, and was dropped because the delegate only acted once `launched` was set. Those paths now go into a main-thread queue that did_finish_launching dispatches as OpenCommandScript events.

## Tried and ruled out
Opening the file in place of the startup window: the GUI creates its first window (async_run_terminal_gui / spawn_tab_in_domain_if_mux_is_empty) before did_finish_launching, so it already exists when the path is known. Replacing or closing it would also undo whatever a gui-startup handler set up, so the file opens in a second window instead.

## In the code:
window/src/os/macos/app.rs PENDING_OPEN_FILES, application_open_file, application_did_finish_launching. Test cold start with `open -a TEST.app --env WEZTERM_CONFIG_FILE=... --env WEZTERM_LOG=window=debug DIR`.
