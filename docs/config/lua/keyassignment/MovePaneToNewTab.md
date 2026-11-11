# `MovePaneToNewTab`

{{since('nightly')}}

Moves the active pane out of its tab into a new tab in the same window,
like kitty's `detach_window` and iTerm2's "Move Session to New Tab". The rest of
the tab's panes stay where they are.

If the pane is alone in its tab, nothing happens.

This action is in the Window menu and the command palette. It has no
default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 't',
    mods = 'CMD|OPT',
    action = wezterm.action.MovePaneToNewTab,
  },
}
```

To pick which pane to move, use [PaneSelect](PaneSelect.md) with
`mode = 'MoveToNewTab'`.
