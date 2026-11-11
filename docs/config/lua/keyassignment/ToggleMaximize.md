# `ToggleMaximize`

{{since('nightly')}}

Switches the window between filling the screen and its previous size,
the same as the green Zoom button with Option held. Unlike
[ToggleFullScreen](ToggleFullScreen.md) it keeps the menu bar, the Dock
and the window title bar. This is like kitty's `toggle_maximized`.

It is in the Window menu as "Zoom" and has no default key binding.

```lua
config.keys = {
  {
    key = 'm',
    mods = 'CMD|SHIFT',
    action = wezterm.action.ToggleMaximize,
  },
}
```
