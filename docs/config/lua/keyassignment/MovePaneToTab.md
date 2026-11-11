# `MovePaneToTab`

{{since('nightly')}}

Moves the active pane into another tab in the same window. It is split
to the right of that tab's active pane and gets the focus. If the pane
was the only one in its tab, that tab closes. This is like moving a
window to another tab with kitty's `detach_window`.

The argument is the tab index, counting from 0. A negative number
counts from the right, as with [ActivateTab](ActivateTab.md): `-1` is
the right-most tab. Moving a pane into its own tab does nothing.

```lua
config.keys = {}
for i = 1, 8 do
  -- CMD+SHIFT+number moves the active pane into that tab
  table.insert(config.keys, {
    key = tostring(i),
    mods = 'CMD|SHIFT',
    action = wezterm.action.MovePaneToTab(i - 1),
  })
end
```

See also [MovePaneToNewTab](MovePaneToNewTab.md) and
[MovePaneToNewWindow](MovePaneToNewWindow.md).
