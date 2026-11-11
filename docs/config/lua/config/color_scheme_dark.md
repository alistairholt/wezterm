---
tags:
  - appearance
  - color
---
# `color_scheme_dark`

{{since('nightly')}}

The color scheme to use while macOS is in dark mode. WezTerm switches
to it as soon as the system appearance changes, with no config reload
needed. Set it together with
[color_scheme_light](color_scheme_light.md):

```lua
config.color_scheme_light = 'Builtin Solarized Light'
config.color_scheme_dark = 'Builtin Solarized Dark'
```

When it isn't set, `color_scheme` is used in dark mode. The names are
the same as for `color_scheme`: see
[the list of color schemes](../../../colorschemes/index.md).

This does the same as the
[wezterm.gui.get_appearance()](../wezterm.gui/get_appearance.md) example
without any Lua. In a remote multiplexer domain the server's config
decides the colors, and the server doesn't know the appearance, so there
`color_scheme` is used.
