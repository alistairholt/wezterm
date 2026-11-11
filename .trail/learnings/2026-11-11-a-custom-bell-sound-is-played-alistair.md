---
title: A custom bell sound is played with NSSound and cached per name
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 11957 }
id: l-f4561d8e
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/termwindow/mod.rs
---

## Decided
audible_bell_sound (kitty's bell_path) plays through NSSound. A value with a slash is treated as a file path (initWithContentsOfFile:byReference:). Anything else is a system sound name (soundNamed:, which looks in /System/Library/Sounds and the user's sound folders). Loaded sounds are kept per name in a thread_local, including failures, so a bad name logs one error and then the system beep plays. Each bell calls stop then play, because play on a sound that is still playing does nothing and a quick run of bells would be silent.

## Watch out
Tests can't listen. Check the "playing sound ...: isPlaying=true" debug line from window::os::macos::sound.

In the code: window/src/os/macos/sound.rs; the Alert::Bell arm in wezterm-gui/src/termwindow/mod.rs.
