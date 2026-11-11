---
tags:
  - tab_bar
---
# `fancy_tab_style = "Default"`

{{since('nightly')}}

Sets how the [fancy tab bar](use_fancy_tab_bar.md) draws its tabs.

* `"Default"`: tabs have rounded tops, and the active tab joins the pane
  below it.
* `"Flat"`: tabs have no shape of their own. They sit on the tab bar,
  split by thin dividers, and the active tab stands out by its text color.
* `"Capsule"`: as `"Flat"`, but the active tab is a rounded pill in the
  active tab's background color, with no dividers next to it.

Flat and capsule tabs take their colors from
[colors.tab_bar](../../appearance.md#tab-bar-appearance-colors):

* The bar behind the tabs is `window_frame.active_titlebar_bg` (or
  `inactive_titlebar_bg` when the window isn't focused).
* Active tab text uses `active_tab.fg_color`, and the capsule uses
  `active_tab.bg_color`.
* Inactive tab text uses `inactive_tab.fg_color`, and
  `inactive_tab_hover.fg_color` under the mouse.
* Dividers use `inactive_tab_edge`.

A background or foreground color set at the start of the text that
[format-tab-title](../window-events/format-tab-title.md) returns still
overrides these.

```lua
config.use_fancy_tab_bar = true
config.fancy_tab_style = 'Capsule'
```
