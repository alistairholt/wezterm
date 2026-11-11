---
title: "Balancing panes counts the panes side by side in each split's direction"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 16, tokens: 260311 }
id: l-95e91126
scope: repo
status: active
src: agent
touches:
  - config/src/keyassignment.rs
  - docs/changelog.md
  - docs/config/lua/keyassignment/ToggleTabBar.md
  - wezterm-gui/src/commands.rs
  - wezterm-gui/src/termwindow/mod.rs
  - docs/config/lua/keyassignment/CloseOtherPanes.md
  - docs/config/lua/keyassignment/RenameTab.md
  - wezterm-gui/src/overlay/prompt.rs
  - docs/config/lua/keyassignment/CloseOtherTabs.md
  - docs/config/default-keys.md
---

## Decided
BalancePanes sizes each split from how many panes sit side by side in that split's direction, not 50/50 per split. A split in the other direction counts as wide as its widest child. 50/50 per split gives three columns of 50%, 25%, 25% when one column was split from another.

Each split's divider takes one cell. For a split with n panes across it, n - 1 dividers are taken out, the rest is shared equally, and the left side gets its share of panes plus the dividers inside it.

## Watch out
It skips zoomed tabs, like adjust_pane_size, because applying sizes then would resize panes hidden behind the zoom.

In the code: panes_across, balance_splits and TabInner::balance_panes in mux/src/tab.rs.
