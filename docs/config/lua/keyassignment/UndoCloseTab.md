# `UndoCloseTab`

{{since('nightly')}}

Brings back the most recently closed tab, as iTerm2's "Undo Close Tab"
does. The tab returns to the window and position it was closed from,
and becomes the active tab.

A closed tab can be brought back for
[undo_close_tab_seconds](../config/undo_close_tab_seconds.md) (5 by
default). Until then its programs keep running in the background; after
that they are killed as usual. Run the action again to bring back the
tab closed before that one.

Tabs closed with [CloseCurrentTab](CloseCurrentTab.md) or the tab's
close button can be brought back. Closing the last tab in a window,
[CloseOtherTabs](CloseOtherTabs.md) and closing a whole window can't be
undone.

This action is in the Shell menu and the command palette. It has no
default key binding. To use `CMD-z` as iTerm2 does:

```lua
config.keys = {
  {
    key = 'z',
    mods = 'CMD',
    action = wezterm.action.UndoCloseTab,
  },
}
```
