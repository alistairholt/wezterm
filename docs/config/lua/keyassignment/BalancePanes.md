# `BalancePanes`

{{since('nightly')}}

Resizes the panes in the active tab so the panes in each row and column
get the same space, like iTerm's Arrange Split Panes Evenly. For
example, three panes side by side each get a third of the width, even
if one of them was split from the other two.

It does nothing while a pane is zoomed. It is in the Window > Resize
Pane menu and has no default key binding.

```lua
config.keys = {
  {
    key = '=',
    mods = 'CMD|OPT',
    action = wezterm.action.BalancePanes,
  },
}
```
