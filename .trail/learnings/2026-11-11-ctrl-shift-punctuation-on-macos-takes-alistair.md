---
title: Ctrl+Shift punctuation on macOS takes the key from charactersIgnoringModifiers
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 8, tokens: 132738 }
id: l-0c7de9ee
scope: repo
status: active
src: agent
touches:
  - window/src/os/macos/window.rs
  - docs/changelog.md
  - wezterm-input-types/src/lib.rs
---

## Decided
On macOS, Ctrl+Shift+<punctuation or digit> arrives with `characters` = the unshifted char and `charactersIgnoringModifiers` = the shifted one (Ctrl+Shift+, gives chars="," unmod="<"). key_common then took chars, saw chars != unmod, kept unmod only as the raw key and cleared the modifiers, so the key went out as a bare ",". Any Ctrl+Shift combo where both are single ASCII chars, chars isn't a letter or control char, and unmod is punctuation now takes the key from unmod and keeps Ctrl+Shift, generalising the old Ctrl+Shift+/ special case (#1907). Fixes #6994.

Captured on macOS 26, US layout, by building key events in memory with CGEvent(keyboardEventSource:virtualKey:keyDown:) and NSEvent(cgEvent:) (nothing posted to any app). Affected: , . / = ` and 1 (Ctrl+Shift+2/6/-/[/]/\ give control chars and go through the existing CTRL-A logic; ; and ' already agree).

Separately, the kitty encoder ignored flag 8 (report all keys as escape codes) for text keys: the legacy-text shortcut ran even with flag 8, so "j" went out as "j" and Ctrl+a as ^A. #4003.

## Watch out
- Legacy (non-kitty) output changes too: Ctrl+Shift+, now sends "<" instead of "," (termwiz's encoder ignores Ctrl on "<"), matching xterm.
- Not tried with real keypresses: the app can only be clicked from this setup, not typed into. The unit tests use the captured chars/unmod pairs.
- The raw-key pass (phys/raw code bindings) runs before this and is unchanged, so default bindings like CTRL+SHIFT+1 still match there.

In the code: window/src/os/macos/window.rs (ctrl_shift_dropped_shift, swap_unmod_and_chars in key_common, test ctrl_shift_takes_shifted_key_from_unmod); wezterm-input-types/src/lib.rs (use_legacy in encode_kitty, test encode_issue_4003_report_all_keys).
