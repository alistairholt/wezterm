---
title: The scrollback pager reads a temp file on stdin that is deleted before the pager starts
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 3, tokens: 34059 }
id: l-96d20268
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
ShowScrollbackInPager writes the pane's lines (lines_to_escapes, trailing blank rows trimmed) to a 0600 tempfile and spawns `/bin/sh -c 'exec <"$1" && rm -f -- "$1" && shift && exec "$@"' sh FILE PAGER...` in a new tab. The pager gets the file as a seekable stdin, so `less +G` works, and the file is gone before the pager starts.

## Tried and ruled out
Passing the path as an argument and removing it after the pager exits (`"$@" "$f"; rm -f "$f"`): a SIGTERM to less left the file behind, and closing the tab (SIGHUP to the shell) would too. Scrollback can hold secrets, so it must not linger in $TMPDIR.

## Watch out
To test it without keypresses, bind the action to the `bell` event in a test config and have default_prog print colored lines and then `\a`.

## In the code:
wezterm-gui/src/termwindow/mod.rs show_scrollback_in_pager; config scrollback_pager (default less +G -R -S).
