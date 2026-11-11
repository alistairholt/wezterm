---
tags:
  - bell
---
# `audible_bell_sound`

{{since('nightly')}}

The sound to play for the audible bell, instead of the system beep.
This is like kitty's `bell_path` and iTerm2's bell sound setting. It is
not set by default.

It can be the name of a macOS system sound:

```lua
config.audible_bell_sound = 'Glass'
```

The system sounds are in `/System/Library/Sounds`, for example
`Basso`, `Funk`, `Glass`, `Ping`, `Pop` and `Tink`.

Or the path of a sound file:

```lua
config.audible_bell_sound = '/Users/me/Sounds/ding.aiff'
```

It only plays when [audible_bell](audible_bell.md) is `"SystemBeep"`,
the default. If the sound can't be loaded, WezTerm logs an error once
and uses the system beep.
