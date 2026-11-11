---
tags:
  - appearance
  - mouse
---
# `command_status_marks = true`

{{since('nightly')}}

Draws a mark in the gutter to the left of each prompt whose command has
finished, like iTerm2's marks. The mark is green if the command
succeeded (exit status 0) and red if it failed. The colors are ANSI
green and red from your color scheme.

Click a mark to select that command's output, then press CMD-c to copy
it. If the output has scrolled out of view, scroll back to its prompt
first.

Marks need [shell integration](../../../shell-integration.md), or any
shell setup that reports each command's exit status with `OSC 133;D`
after marking its prompt with `OSC 133;A`. They aren't shown for panes
in a remote multiplexer domain.

The mark sits in the window padding, so it needs
[window_padding](window_padding.md) `left` to be at least a few pixels
wide; the default is one cell. With less room it is drawn over the
first column and can't be clicked.

To turn the marks off:

```lua
config.command_status_marks = false
```
