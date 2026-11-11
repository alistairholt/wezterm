# `confirm_multiline_paste = true`

{{since('nightly')}}

When you paste text that contains a line break into a program that
hasn't turned on bracketed paste, WezTerm asks before sending it. Such
a program, for example a shell without bracketed paste support or a
`cat` waiting for input, runs each pasted line as soon as it arrives,
so a paste with a stray command can't be taken back. This is like
kitty's `paste_actions confirm` and iTerm2's paste warning.

Programs that turn on bracketed paste, such as current versions of
zsh, bash, fish and most editors, get the paste straight away. Panes in
remote multiplexer domains are never asked about, because WezTerm can't
tell whether bracketed paste is on there.

Set it to `false` to paste without asking:

```lua
config.confirm_multiline_paste = false
```
