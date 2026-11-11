# `OpenFileAtMouseCursor`

{{since('nightly')}}

Opens the file named by the text under the mouse cursor, like iTerm2's
semantic history. It is bound to CMD-click by default.

If the mouse is over a link, the link is opened, as with
[OpenLinkAtMouseCursor](OpenLinkAtMouseCursor.md).

Otherwise the action takes the text around the mouse cursor up to the
next space, quote or bracket, and treats it as a path:

* A path starting with `~/` is looked up in your home directory.
* A relative path is looked up in the pane's current directory. This
  needs [shell integration](../../../shell-integration.md), or another
  way for the shell to report its directory with OSC 7.
* A location after the path is read as a line and column, as in
  `src/main.rs:12:5` from a compiler or `src/main.rs:12:` from
  `grep -n`. Python's `File "app.py", line 12` is also read.
* The `a/` and `b/` that `git diff` puts before paths are dropped if the
  file isn't found with them.

Nothing happens unless the file or directory exists. Paths with spaces
aren't found.

The file is opened with [open_file_command](../config/open_file_command.md),
or with macOS's `open` if that isn't set.

The default binding:

```lua
config.mouse_bindings = {
  {
    event = { Up = { streak = 1, button = 'Left' } },
    mods = 'CMD',
    action = wezterm.action.OpenFileAtMouseCursor,
  },
}
```
