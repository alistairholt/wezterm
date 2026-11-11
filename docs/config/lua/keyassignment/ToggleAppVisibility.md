# `ToggleAppVisibility`

{{since('nightly')}}

Hides WezTerm when it is the active app. Otherwise it brings WezTerm to
the front, opening a window if there is none.

This is what [macos_global_hotkey](../config/macos_global_hotkey.md)
does. Bound to a normal key, it hides WezTerm, like
[HideApplication](HideApplication.md).

```lua
config.keys = {
  {
    key = 'Escape',
    mods = 'CTRL|OPT',
    action = wezterm.action.ToggleAppVisibility,
  },
}
```
