# `ShowScrollbackInPager`

{{since('nightly')}}

Opens the current pane's scrollback and screen in a pager in a new tab,
like kitty's `show_scrollback`. Colors and other attributes are kept.
This gives you the pager's own search and navigation over the whole
history. Quit the pager to close the tab.

The pager is set by [scrollback_pager](../config/scrollback_pager.md),
which defaults to `less +G -R -S`.

This action is in the Edit menu and the command palette. It has no
default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 'h',
    mods = 'CMD|SHIFT',
    action = wezterm.action.ShowScrollbackInPager,
  },
}
```
