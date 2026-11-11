# `SaveScrollbackToFile`

{{since('nightly')}}

Asks where to save, then writes the current pane's scrollback and
screen to that file as plain text. This is like iTerm2's "Save
Contents". Colors and other attributes are not saved.

The file name starts as `WezTerm` followed by the date and time.

This action is in the Shell menu as "Save contents..." and in the
command palette. It has no default key binding. You can bind it like
this:

```lua
config.keys = {
  {
    key = 's',
    mods = 'CMD|SHIFT',
    action = wezterm.action.SaveScrollbackToFile,
  },
}
```

See also [ShowScrollbackInPager](ShowScrollbackInPager.md).
