---
tags:
  - bell
---
# `window_alert_on_bell = true`

{{since('nightly')}}

When a bell rings in a pane while WezTerm isn't the active app, bounce
WezTerm's Dock icon once to get your attention. This matches kitty's
option of the same name. It has no effect while WezTerm is active.

Set it to `false` to turn this off:

```lua
config.window_alert_on_bell = false
```

See also [audible_bell](audible_bell.md), [visual_bell](visual_bell.md)
and the [bell event](../window-events/bell.md).
