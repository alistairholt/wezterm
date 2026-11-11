---
tags:
  - tab_bar
---
# `tab_activity_symbol = ""`

{{since('nightly')}}

Text to show before a tab's title when a pane in that tab has printed
output since you last looked at it. It goes away when you switch to the
tab. This matches kitty's option of the same name. It is empty, and so
off, by default:

```lua
config.tab_activity_symbol = '● '
```

When a bell has also rung in the tab, [bell_on_tab](bell_on_tab.md) is
shown instead. It only applies to the default tab titles. If you set
titles yourself with the
[format-tab-title](../window-events/format-tab-title.md) event, check
`tab.has_unseen_output` from [TabInformation](../TabInformation.md).
