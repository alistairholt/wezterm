# `CopyOrInterrupt`

{{since('nightly')}}

Copies the selection to the clipboard if there is one. Otherwise it
sends Ctrl-C to the pane, which usually stops the running program. This
is like kitty's `copy_or_interrupt`, and lets one key do both jobs.

It has no default key binding. To make Ctrl-C copy when text is
selected:

```lua
config.keys = {
  {
    key = 'c',
    mods = 'CTRL',
    action = wezterm.action.CopyOrInterrupt,
  },
}
```
