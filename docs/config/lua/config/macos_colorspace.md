---
tags:
  - appearance
  - color
---
# `macos_colorspace = "sRGB"`

{{since('nightly')}}

Tells macOS which color space WezTerm's colors are in, so it can convert
them for your display. Terminal colors, such as `#ff0000` in a color
scheme, are sRGB colors, the same as on the web.

The possible values are:

* `"sRGB"` - colors are sRGB. They look the same as in a browser or
  Terminal.app. This is the default.
* `"DisplayP3"` - colors are treated as Display P3. On a wide gamut
  display, such as a recent Mac's built-in screen, they look more
  saturated: `#ff0000` shows as the display's own, deeper red instead of
  sRGB red. On an sRGB display there is little difference.
* `"None"` - colors aren't tagged, and macOS decides how to show them.
  On macOS 27 this looks the same as `"sRGB"`.

```lua
-- More saturated colors on a wide gamut display
config.macos_colorspace = 'DisplayP3'
```

Changes take effect when the config is reloaded.
