# `ToggleBroadcastInput`

{{since('nightly')}}

Turns Broadcast Input on or off for the current tab, like iTerm2's
"Broadcast Input to All Panes in Current Tab". While it is on, what you
type in the active pane is also sent to every other pane in the tab.
That covers key presses, pastes, and the
[SendString](SendString.md) and [SendKey](SendKey.md) actions. Each
pane gets keys encoded for the keyboard protocol it asked for.

The tab's title starts with 📢 while it is on. If you set tab titles
yourself with [format-tab-title](../window-events/format-tab-title.md),
check `tab.broadcast_input` from [TabInformation](../TabInformation.md).

Panes that show an overlay, such as copy mode, don't get the input. While
the active pane shows an overlay, nothing is broadcast.

This action is in the Shell menu and the command palette. It has no
default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 'i',
    mods = 'CMD|ALT',
    action = wezterm.action.ToggleBroadcastInput,
  },
}
```
