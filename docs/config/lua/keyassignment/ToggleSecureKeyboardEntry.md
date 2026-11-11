# `ToggleSecureKeyboardEntry`

{{since('nightly')}}

Turns macOS Secure Keyboard Entry on or off, like the menu item of the
same name in Terminal.app and iTerm2. While it is on and WezTerm is the
active app, other apps can't read what you type through keyboard event
taps. WezTerm turns it off again whenever it stops being the active app.

This action is in the WezTerm menu, with a checkmark while it is on. It
has no default key binding. You can bind it like this:

```lua
config.keys = {
  {
    key = 's',
    mods = 'CMD|SHIFT',
    action = wezterm.action.ToggleSecureKeyboardEntry,
  },
}
```

To turn it on at startup, see
[macos_secure_keyboard_entry](../config/macos_secure_keyboard_entry.md).
