# `CopyLastCommandOutput`

{{since('nightly')}}

Copies the output of the last command that printed something in the
current pane to the clipboard, as plain text. This is like iTerm2's
"Copy Output of Last Command". The prompt and the command line are left
out.

It needs [shell integration](../../../shell-integration.md), which marks
where each command's output starts and ends.

This action is in the Edit menu and the command palette. It has no
default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 'c',
    mods = 'CMD|ALT',
    action = wezterm.action.CopyLastCommandOutput,
  },
}
```

See also [SelectLastCommandOutput](SelectLastCommandOutput.md) and
[ShowLastCommandOutputInPager](ShowLastCommandOutputInPager.md).
