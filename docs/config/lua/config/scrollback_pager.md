# `scrollback_pager = { "less", "+G", "-R", "-S" }`

{{since('nightly')}}

The command, with its arguments, that
[ShowScrollbackInPager](../keyassignment/ShowScrollbackInPager.md) runs.
It reads the scrollback on its standard input, with escape sequences
for colors and attributes, so it needs to pass those through. The
default starts `less` at the end of the text (`+G`), keeps colors
(`-R`) and doesn't wrap long lines (`-S`).

The command is run directly, not through a shell, so give the full path
if it isn't on WezTerm's `PATH`:

```lua
config.scrollback_pager = { '/opt/homebrew/bin/moar' }
```
