---
title: Opening a shell script from Finder depends on the shell keeping typed-ahead input
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 1, tokens: 1580 }
id: l-168e86a3
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/frontend.rs
---

## Watch out
OpenCommandScript spawns the default shell and writes `'SCRIPT' ; exit` to it straight away, because running the script directly would skip the user's rc files and macOS gives apps a bare PATH. On this machine's zsh setup the line is echoed but then dropped when the prompt starts, so the script never runs. That is old behavior, not caused by the folder fix. A fix would need to wait for the shell's first prompt (OSC 133 or output) before writing, or run the script with `$SHELL -l -c` instead.

## Decided
A folder sent to application:openFile: now spawns a window with cwd set to the folder and writes nothing, because Info.plist lists folders as a document type and typing a directory path into a shell fails with "permission denied".

## In the code:
wezterm-gui/src/frontend.rs app_event_handler, ApplicationEvent::OpenCommandScript. Test with a bundled copy of the build that uses the real Info.plist under another bundle id, registered with lsregister -f, and `open -a APP PATH`.
