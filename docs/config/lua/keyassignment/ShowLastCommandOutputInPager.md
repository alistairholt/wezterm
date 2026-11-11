# `ShowLastCommandOutputInPager`

{{since('nightly')}}

Opens the output of the last command that printed something in the
current pane, in a pager in a new tab. This is like kitty's
`show_last_non_empty_command_output`. Colors and other attributes are
kept. Quit the pager to close the tab.

It needs [shell integration](../../../shell-integration.md), which marks
where each command's output starts and ends. Without it there is no
command output to find, and nothing opens.

The pager is set by [scrollback_pager](../config/scrollback_pager.md),
which defaults to `less +G -R -S`.

This action is in the Edit menu and the command palette. It has no
default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 'g',
    mods = 'CMD|SHIFT',
    action = wezterm.action.ShowLastCommandOutputInPager,
  },
}
```

See also [ShowScrollbackInPager](ShowScrollbackInPager.md).
