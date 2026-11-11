---
tags:
  - keys
---
# `macos_secure_keyboard_entry = false`

{{since('nightly')}}

When `true`, WezTerm turns on macOS Secure Keyboard Entry at startup.
While it is on and WezTerm is the active app, other apps can't read
what you type through keyboard event taps. Some apps that watch the
keyboard, such as text expanders, stop working in WezTerm while it is
on.

You can also turn it on and off from the WezTerm menu or with
[ToggleSecureKeyboardEntry](../keyassignment/ToggleSecureKeyboardEntry.md).
Reloading the config only changes the setting when this option's value
has changed, so a choice made from the menu stays.

```lua
config.macos_secure_keyboard_entry = true
```
