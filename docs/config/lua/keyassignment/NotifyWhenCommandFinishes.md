# `NotifyWhenCommandFinishes`

{{since('nightly')}}

Shows a desktop notification once, when the command running in the
current pane finishes. If no command is running, it waits for the next
one. This is like iTerm2's "Alert on Next Mark". Run the action again
before the command finishes to cancel it.

It works whatever
[notify_on_command_finish](../config/notify_on_command_finish.md) is set
to, and however long the command runs.

It needs [shell integration](../../../shell-integration.md), which tells
WezTerm when each command starts and ends.

This action is in the Shell menu and the command palette. It has no
default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 'a',
    mods = 'CMD|OPT',
    action = wezterm.action.NotifyWhenCommandFinishes,
  },
}
```
