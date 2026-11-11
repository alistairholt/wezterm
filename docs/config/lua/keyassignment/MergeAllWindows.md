# `MergeAllWindows`

{{since('nightly')}}

Moves the tabs of every other window in the current workspace into the
current window, and closes the windows left empty, like "Merge All
Windows" in Terminal.app and iTerm2. The tab you were using stays
active.

This action is in the Window menu and the command palette. It has no
default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 'm',
    mods = 'CMD|ALT',
    action = wezterm.action.MergeAllWindows,
  },
}
```

See also [MoveTabToNewWindow](MoveTabToNewWindow.md).
