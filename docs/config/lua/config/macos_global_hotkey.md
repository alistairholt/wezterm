---
tags:
  - keys
---
# `macos_global_hotkey`

{{since('nightly')}}

A system-wide hotkey that shows and hides WezTerm, like iTerm2's
hotkey. It works whichever app you are in:

* When WezTerm is the active app, the hotkey hides it.
* Otherwise it brings WezTerm to the front. If no window is open, it
  opens one.

It is not set by default. Set it with the same `key` and `mods` fields
as a [key binding](../../../config/keys.md):

```lua
config.macos_global_hotkey = { key = 'Space', mods = 'CTRL|OPT' }
```

The hotkey goes through macOS's hotkey service, so it doesn't need the
Accessibility permission. If another app already uses the same key and
modifiers, WezTerm logs an error and the hotkey isn't set.

The same behavior is available as the
[ToggleAppVisibility](../keyassignment/ToggleAppVisibility.md) action.
