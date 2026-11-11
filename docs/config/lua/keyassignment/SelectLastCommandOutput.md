# `SelectLastCommandOutput`

{{since('nightly')}}

Selects the output of the last command that printed something in the
current pane, as iTerm2's "Select Output of Last Command" does. The
prompt, the command line and blank lines at the end of the output are
left out. If the start of the output is off screen, the pane scrolls to
it.

It doesn't copy anything. Press `CMD-c` to copy the selection, or use
[CopyLastCommandOutput](CopyLastCommandOutput.md) to copy in one step.

It needs [shell integration](../../../shell-integration.md), which marks
where each command's output starts and ends.

It is bound to `CMD-SHIFT-a` by default, and is in the Edit menu and the
command palette.
