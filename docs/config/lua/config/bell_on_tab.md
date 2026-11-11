---
tags:
  - bell
  - tab_bar
---
# `bell_on_tab = "🔔 "`

{{since('nightly')}}

When a bell rings in a tab that isn't the focused tab, or while the
window isn't focused, WezTerm puts this text before the tab's title in
the tab bar. It goes away when you switch to that tab in a focused
window. This matches kitty's option of the same name.

Set it to an empty string to turn this off:

```lua
config.bell_on_tab = ''
```

It only applies to the default tab titles. If you set titles yourself
with the [format-tab-title](../window-events/format-tab-title.md)
event, check `tab.has_unseen_bell` from
[TabInformation](../TabInformation.md) instead. That field is set even
when `bell_on_tab` is empty.

See also [macos_dock_badge_on_bell](macos_dock_badge_on_bell.md),
[window_alert_on_bell](window_alert_on_bell.md),
[audible_bell](audible_bell.md) and [visual_bell](visual_bell.md).
