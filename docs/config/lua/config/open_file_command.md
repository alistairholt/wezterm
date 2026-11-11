---
tags:
  - mouse
---
# `open_file_command`

{{since('nightly')}}

The command, with its arguments, that
[OpenFileAtMouseCursor](../keyassignment/OpenFileAtMouseCursor.md)
runs to open a file. In each argument, `{file}` is replaced with the
file's full path, `{line}` with the line number and `{column}` with the
column. When the text gave no line or column, they are `1`.

When it isn't set, the file is opened with macOS's `open`, so it opens
in the app Finder would use.

The command runs through your login shell, so programs on the `PATH`
your shell sets up are found. For example, to open files in an editor
at the line:

```lua
-- Visual Studio Code
config.open_file_command = { 'code', '--goto', '{file}:{line}:{column}' }

-- Zed
config.open_file_command = { 'zed', '{file}:{line}:{column}' }

-- Vim in a new WezTerm tab
config.open_file_command =
  { 'wezterm', 'cli', 'spawn', '--', 'vim', '+{line}', '{file}' }
```
