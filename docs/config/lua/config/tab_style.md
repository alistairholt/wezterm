---
tags:
  - tab_bar
---
# `tab_style = "Fancy"`

{{since('nightly')}}

Sets how the tab bar looks.

* `"Retro"`: one row of text in the main terminal font, styled with
  [tab_bar_style](tab_bar_style.md).
* `"Fancy"`: drawn with the [window_frame](window_frame.md) font. Tabs
  have rounded tops, and the active tab joins the pane below it.
* `"Flat"`: as `"Fancy"`, but tabs have no shape of their own. They sit in
  the middle of the tab bar, split by thin dividers, and the active tab
  stands out by its text color.
* `"Capsule"`: as `"Flat"`, but the active tab is a pill with rounded
  ends in the active tab's background color, with no dividers next to it.

When `tab_style` isn't set, [use_fancy_tab_bar](use_fancy_tab_bar.md)
picks `"Fancy"` (the default) or `"Retro"`. When both are set,
`tab_style` wins.

```lua
config.tab_style = 'Capsule'
```

## Padding and corners

`window_frame` sets the space around each tab's title (`tab_padding_x`
applies to Fancy tabs too) and how round the Capsule pill is:

```lua
config.window_frame = {
  tab_padding_x = '0.75cell',
  tab_padding_y = '0.25cell',
  -- A fraction of the tab's height: 0.5 gives round ends, 0 square corners
  tab_corner_radius = 0.5,
}
```

The values shown are the defaults. The pill's corners are superellipses,
like macOS's own rounded shapes, rather than circular arcs.

## Colors

Flat and Capsule tabs take their colors from
[colors.tab_bar](../../appearance.md#tab-bar-appearance-colors):

* The bar behind the tabs is `window_frame.active_titlebar_bg` (or
  `inactive_titlebar_bg` when the window isn't focused).
* Active tab text uses `active_tab.fg_color`, and the pill uses
  `active_tab.bg_color`.
* Inactive tab text uses `inactive_tab.fg_color`, and
  `inactive_tab_hover.fg_color` under the mouse.
* Dividers use `inactive_tab_edge`.
* With [show_close_tab_button_in_tabs](show_close_tab_button_in_tabs.md),
  the close button is a small X in the tab's text color. Under the mouse
  it gets a rounded square behind it, in that color at low opacity.

In the Flat and Capsule styles, a `│` in the left or right status is drawn
as the same line as the dividers between tabs, in the color it was given,
so status sections can be split the same way as tabs.

A background or foreground color set at the start of the text that
[format-tab-title](../window-events/format-tab-title.md) returns still
overrides these.
