# `MoveTabToNewWindow`

{{since('nightly')}}

Moves the active tab out of the current window into a new window of its
own, like "Move Tab to New Window" in Terminal.app and iTerm2. Nothing
happens when the tab is the only one in its window.

This action is in the Window menu and the command palette. It has no
default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 'n',
    mods = 'CMD|ALT',
    action = wezterm.action.MoveTabToNewWindow,
  },
}
```

See also [MergeAllWindows](MergeAllWindows.md).
