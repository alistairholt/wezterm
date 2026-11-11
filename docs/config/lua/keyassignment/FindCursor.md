# `FindCursor`

{{since('nightly')}}

Briefly draws a crosshair through the cursor's row and column so you
can spot the cursor in a large window, as iTerm2's "Find Cursor" does.
The crosshair uses the cursor color and fades out after about a second.
If you have scrolled back, the pane scrolls to the bottom first.

It is bound to `CMD-/` by default, and is in the View menu and the
command palette.

To keep a bar behind the cursor's row all the time, see
[cursor_guide_color](../config/cursor_guide_color.md).
