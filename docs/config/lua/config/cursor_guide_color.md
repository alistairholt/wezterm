---
tags:
  - appearance
  - cursor
---
# `cursor_guide_color`

{{since('nightly')}}

When set, WezTerm draws a bar of this color behind the cursor's row in
the active pane, so the cursor is easy to find. This is like iTerm2's
cursor guide. It is not set by default.

Use a mostly transparent color, so the text stays easy to read:

```lua
config.cursor_guide_color = 'rgba(120, 160, 255, 0.25)'
```

The bar is drawn behind the text. Cells that have their own background
color are drawn over it. It isn't shown when the cursor is hidden, or
when you have scrolled back so the cursor's row isn't on screen.
