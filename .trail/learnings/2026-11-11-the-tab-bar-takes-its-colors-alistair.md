---
title: The tab bar takes its colors from the color scheme unless the user set them
author: Alistair
date: 2026-11-11
branch: claude/context-menu
cost: { minutes: 6, tokens: 139523 }
id: l-84acbaea
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - config/src/keyassignment.rs
  - config/src/lib.rs
  - docs/changelog.md
  - docs/config/appearance.md
  - docs/config/lua/config/color_scheme_dark.md
  - docs/config/lua/config/color_scheme_light.md
  - docs/config/lua/keyassignment/ShowContextMenu.md
  - docs/config/mouse.md
  - wezterm-gui/src/commands.rs
---

## Decided
With a color scheme set, tab bar colors are worked out from the scheme's background and foreground, then the scheme's own tab_bar colors and `colors.tab_bar` go on top. Without a scheme the stock dark greys stay, so the default look doesn't change.

## Watch out
- The fancy tab bar used to read only `config.colors.tab_bar` and ignored the scheme's tab bar colors entirely. It now reads `resolved_palette.tab_bar`, which already has `colors` overlaid.
- `window_frame` titlebar colors aren't Options, so "unset" means "equal to the built-in default". A user who sets exactly #333333 gets the scheme color instead.

In the code: TabBarColors::from_scheme and WindowFrameConfig::follow_tab_bar in config/src/color.rs; the call is in compute_extra_defaults in config/src/config.rs; build_fancy_tab_bar in wezterm-gui/src/termwindow/render/fancy_tab_bar.rs.
