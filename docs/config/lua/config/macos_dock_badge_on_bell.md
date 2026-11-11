---
tags:
  - bell
---
# `macos_dock_badge_on_bell = true`

{{since('nightly')}}

Show a badge on WezTerm's Dock icon with the number of tabs, across all
windows, where a bell rang that you haven't seen yet. A tab counts until
you switch to it in a focused window, the same rule as
[bell_on_tab](bell_on_tab.md). This matches Terminal.app.

Set it to `false` to turn the badge off:

```lua
config.macos_dock_badge_on_bell = false
```

See also [window_alert_on_bell](window_alert_on_bell.md), which bounces
the Dock icon.
