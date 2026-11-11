# `CloseOtherTabs`

{{since('nightly')}}

Closes every tab in the window except the active one, like iTerm's
Close Other Tabs and kitty's `close_other_tabs_in_os_window`.

With `confirm = true`, if any of those tabs would ask before closing
(see [skip_close_confirmation_for_processes_named](../config/skip_close_confirmation_for_processes_named.md)),
you are asked once for all of them. With `confirm = false` they close
straight away.

It is in the Shell menu and has no default key binding.

```lua
config.keys = {
  {
    key = 'w',
    mods = 'CMD|OPT',
    action = wezterm.action.CloseOtherTabs { confirm = true },
  },
}
```
