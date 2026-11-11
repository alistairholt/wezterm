# `CloseOtherPanes`

{{since('nightly')}}

Closes every pane in the active tab except the active one, like kitty's
`close_other_windows_in_tab`. Panes hidden by a zoomed pane are closed
too.

With `confirm = true`, if any of those panes would ask before closing
(see [skip_close_confirmation_for_processes_named](../config/skip_close_confirmation_for_processes_named.md)),
you are asked once for all of them. With `confirm = false` they close
straight away.

It is in the Shell menu and has no default key binding.

```lua
config.keys = {
  {
    key = 'w',
    mods = 'CMD|OPT|SHIFT',
    action = wezterm.action.CloseOtherPanes { confirm = true },
  },
}
```
