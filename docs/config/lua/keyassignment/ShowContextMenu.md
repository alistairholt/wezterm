# `ShowContextMenu`

{{since('nightly')}}

Shows a menu at the mouse pointer, like Terminal.app's right-click menu.
It is bound to a right click by default.

The menu has:

* **Open Link** and **Copy Link**, when the pointer is over a link.
* **Copy** (when text is selected), **Paste** and **Select All**.
* **Split Right** and **Split Down**, which split the pane under the
  pointer.
* **Clear Scrollback** and **Close Pane**.

Right-clicking a pane that isn't active makes it active first, so the
menu acts on that pane.

When a program has turned on mouse reporting, such as vim with
`set mouse=a`, a right click goes to the program instead. Hold SHIFT
(see [bypass_mouse_reporting_modifiers](../config/bypass_mouse_reporting_modifiers.md))
to get the menu.

The default binding:

```lua
config.mouse_bindings = {
  {
    event = { Down = { streak = 1, button = 'Right' } },
    mods = 'NONE',
    action = wezterm.action.ShowContextMenu,
  },
}
```
