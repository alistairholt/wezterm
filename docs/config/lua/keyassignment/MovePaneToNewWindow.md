# `MovePaneToNewWindow`

{{since('nightly')}}

Moves the active pane out of its tab into a new tab in a new window,
like kitty's `detach_window` and iTerm2's "Move Session to New Window". The rest of
the tab's panes stay where they are.

If the pane is alone in its tab, the whole tab moves to the new window,
as [MoveTabToNewWindow](MoveTabToNewWindow.md) does.

This action is in the Window menu and the command palette. It has no
default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 'n',
    mods = 'CMD|OPT',
    action = wezterm.action.MovePaneToNewWindow,
  },
}
```

To pick which pane to move, use [PaneSelect](PaneSelect.md) with
`mode = 'MoveToNewWindow'`.
