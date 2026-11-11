# `ToggleWindowOpacity`

{{since('nightly')}}

Makes a translucent window opaque, and makes it translucent again the
next time. This is like iTerm2's View > Use Transparency. It only does
something when
[window_background_opacity](../config/window_background_opacity.md) is
below `1.0`.

It works by setting `window_background_opacity` to `1.0` in the
window's config overrides (see
[window:set_config_overrides](../window/set_config_overrides.md)), and
removing it again.

This action is in the View menu and the command palette. It has no
default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 'u',
    mods = 'CMD',
    action = wezterm.action.ToggleWindowOpacity,
  },
}
```
