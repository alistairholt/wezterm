---
tags:
  - appearance
---
# `remember_window_size = false`

{{since('nightly')}}

When `true`, new windows open at the size, in rows and columns, of the
last window you resized, instead of
[initial_cols](initial_cols.md) and [initial_rows](initial_rows.md).
This matches kitty's `remember_window_size`.

```lua
config.remember_window_size = true
```

The size is saved in a file named `window-size` in WezTerm's data
folder (`~/Library/Application Support/wezterm` on macOS). It is kept
across restarts.

A window that is maximized or full screen doesn't change the saved
size.

[ResetFontAndWindowSize](../keyassignment/ResetFontAndWindowSize.md)
still resets a window to `initial_cols` and `initial_rows`.
