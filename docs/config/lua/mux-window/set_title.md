## window:set_title(TITLE)

{{since('20220807-113146-c2fee766')}}

Sets the window title to the provided string. The window shows it in
place of the active pane's title until an application changes the title
with an escape sequence. A `format-window-title` event handler, if you have
one, decides the title instead.

```lua
window:set_title 'my title'
```


