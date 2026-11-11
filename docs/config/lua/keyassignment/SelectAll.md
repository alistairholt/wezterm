# `SelectAll`

{{since('nightly')}}

Selects all the text in the active pane: the scrollback and the screen.
Blank rows at the bottom of the screen are left out. Copy the selection
with [CopyTo](CopyTo.md) (`CMD-c`). This is like iTerm's Select All.

It is bound to `CMD-a` by default and is in the Edit menu.

```lua
config.keys = {
  {
    key = 'a',
    mods = 'CMD',
    action = wezterm.action.SelectAll,
  },
}
```
