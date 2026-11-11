---
tags:
  - notifications
---
# `notify_on_command_finish = "Never"`

{{since('nightly')}}

Shows a desktop notification when a command that ran for a long time
finishes. This is like kitty's `notify_on_cmd_finish`.

It needs [shell integration](../../../shell-integration.md), which
tells WezTerm when each command starts and ends.

The possible values are:

* `"Never"` - don't notify. This is the default.
* `"Unfocused"` - notify when the WezTerm window isn't focused.
* `"Invisible"` - notify when the window isn't focused, or when the
  pane isn't on screen (it is in another tab, or another pane is
  zoomed).
* `"Always"` - always notify.

A command only counts as long if it ran for at least
`notify_on_command_finish_after_secs` seconds. The default is `10`.

```lua
config.notify_on_command_finish = 'Unfocused'
config.notify_on_command_finish_after_secs = 30
```

The notification goes through the same system as
[notification_handling](notification_handling.md), so macOS must allow
WezTerm to show notifications.

To be told about just one command, use
[NotifyWhenCommandFinishes](../keyassignment/NotifyWhenCommandFinishes.md).
