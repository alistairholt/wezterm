---
title: "Shell scripts opened from Finder run through the login shell, not typed into a new shell"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 1, tokens: 10306 }
id: l-98e0b56d
scope: repo
status: active
replaces: l-168e86a3
src: agent
touches:
  - wezterm-gui/src/frontend.rs
---

## Decided
OpenCommandScript now spawns `$SHELL -l -i -c 'SCRIPT'` (shell from CommandBuilder::get_shell), because typing `'SCRIPT' ; exit` into a fresh shell failed on this machine's zsh: the typed-ahead line was echoed and then dropped when the prompt started, so the script never ran. -l -i still loads the rc files, which matters because macOS starts apps with a bare PATH. A folder (Info.plist lists folders as a document type) spawns a window with cwd set to it and no command, because running a directory fails with "permission denied".

## Watch out
Test with a bundled copy of the build that uses the real Info.plist under another bundle id and name, registered with lsregister -f, then `open -a APP PATH`. A script that prints whether $PATH contains homebrew shows the rc files loaded. Unregister with lsregister -u afterwards.

## In the code:
wezterm-gui/src/frontend.rs app_event_handler, ApplicationEvent::OpenCommandScript.
