---
title: "Kitty keyboard: keep associated text on Ctrl combinations, and lone modifier keys skip the Option bypass"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 31149 }
id: l-a1d9d4dd
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - docs/cli/cli/set-window-title.md
  - docs/config/lua/mux-window/set_title.md
  - mux/src/localpane.rs
  - mux/src/window.rs
  - wezterm-client/src/domain.rs
  - wezterm-gui/src/termwindow/mod.rs
  - wezterm-gui/src/termwindow/keyevent.rs
  - wezterm-input-types/src/lib.rs
---

## Decided
A review suggested dropping the associated text (flag 16) from Ctrl+key reports, saying kitty sends none. Upstream's own test `encode_issue_3484` expects Ctrl+ф to carry `;1092`, and I couldn't check kitty here, so the text stays.
Ctrl+Shift+` now becomes Ctrl+~ and legacy panes get 0x1e. That matches xterm's Ctrl+~ (termwiz ctrl_mapping), so it was kept.

## Watch out
Lone modifier KeyEvents (sent on macOS since flags_changed began dispatching them) must not enter process_key's Option `bypass_compose` branch, or Left Option goes to copy mode and cancels a pending `f` jump. They also must not be written to the pane while a modal (command palette etc.) is open.

In the code: wezterm-gui/src/termwindow/keyevent.rs process_key (bypass_compose) and key_event_impl modifier early return; wezterm-input-types/src/lib.rs encode_kitty, single_lowercase.
