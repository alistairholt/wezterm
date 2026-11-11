# `RenameTab`

{{since('nightly')}}

Asks for a new title for the active tab, starting from its current
title. Press Enter to set it. Leave it empty to go back to the automatic
title. Press Escape on an empty line to cancel. This is like kitty's
`set_tab_title` and iTerm's Edit Tab Title.

It is in the Shell menu and has no default key binding.

```lua
config.keys = {
  {
    key = 'r',
    mods = 'CMD|SHIFT',
    action = wezterm.action.RenameTab,
  },
}
```

To do something else with the text, use [PromptInputLine](PromptInputLine.md).
