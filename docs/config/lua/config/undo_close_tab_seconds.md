---
tags:
  - tab_bar
---
# `undo_close_tab_seconds = 5`

{{since('nightly')}}

How long, in seconds, a closed tab can be brought back with
[UndoCloseTab](../keyassignment/UndoCloseTab.md). The tab's programs
keep running until the time is up.

Set it to `0` to close tabs at once, with no undo:

```lua
config.undo_close_tab_seconds = 0
```
