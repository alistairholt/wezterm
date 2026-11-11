# `ToggleTabBar`

{{since('nightly')}}

Hides the tab bar if it is shown, and shows it if it is hidden, like
iTerm's View > Show Tab Bar. Showing it works even when
[hide_tab_bar_if_only_one_tab](../config/hide_tab_bar_if_only_one_tab.md)
would hide it. The next toggle goes back to what your config file says.

It applies to the window it is used in and is in the View menu. It has
no default key binding.

```lua
config.keys = {
  {
    key = 't',
    mods = 'CMD|SHIFT',
    action = wezterm.action.ToggleTabBar,
  },
}
```
