---
title: macOS reports lone modifier key presses only to kitty-mode panes
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 4, tokens: 57606 }
id: l-2007efd9
scope: repo
status: active
src: agent
touches:
  - window/src/os/macos/window.rs
  - wezterm-gui/src/termwindow/keyevent.rs
  - docs/changelog.md
  - wezterm-input-types/src/lib.rs
---

## Decided
macOS sends a bare Shift/Control/Option/Command press or release only as flagsChanged, never keyDown/keyUp, so kitty "report all keys" apps never saw them (#5147). flags_changed now also dispatches RawKeyEvent + KeyEvent for the modifier key, with down/up taken from the side-specific NX_DEVICE*KEYMASK bits in modifierFlags (IOLLEvent.h; checked against the SDK). Caps lock is skipped: macOS sends one event per toggle, not a press and a release.

In the GUI, a modifier-only KeyEvent goes to the pane only as kitty encoding, and returns before pop_until_unknown and the termwiz path. Otherwise a bare Shift would cancel a pending copy-mode jump (f then Shift+X rang the bell) and end `until_unknown` key tables.

Kitty encoder (#4916): only unshift via the US table when Shift is really held. "Held" comes from the raw modifiers, because normalize_shift drops SHIFT from upper-case letters; with no raw event an upper-case letter counts as shifted (keeps the #3474 test). The shifted alternate key is only added while Shift is held.

## Watch out
Not tried with real keypresses (the app can only be clicked from here). Unit tests cover the mapping and the encoder; the app was launched and checked to draw.

In the code: window/src/os/macos/window.rs (modifier_key_transition, flags_changed, test modifier_keys_become_key_events); wezterm-gui/src/termwindow/keyevent.rs (is_modifier early return in key_event_impl); wezterm-input-types/src/lib.rs (shift_held / report_shifted in encode_kitty, test encode_issue_4916_unshifted_caret).
