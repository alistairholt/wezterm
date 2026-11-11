---
tags:
  - appearance
---
# `macos_live_resize_repaint_wait_ms = 8`

{{since('nightly')}}

While you drag a window edge or corner on macOS, each step of the resize
changes the size of the panes in the active tab, and full-screen programs
(editors, TUIs) redraw for the new size. If wezterm shows a step before the
program has redrawn, you briefly see the old layout cut off at the window
edge.

This option sets how long, in milliseconds, each live-resize step may wait
for the active tab's panes to redraw before the frame is shown. wezterm
stops waiting early when:

* the program's output has been applied and then stays quiet for about 1ms,
* the program ends a synchronized output block (DEC mode 2026), or
* the program writes nothing within about 1.5ms of the resize (for example
  an idle shell prompt).

A program that writes without pause uses the whole budget on every step.

Set it to `0` to turn the wait off.

```lua
config.macos_live_resize_repaint_wait_ms = 8
```

This works best with [front_end](front_end.md) set to `"WebGpu"`, which
presents each live-resize step in step with the window. It applies to local
panes only; panes in a remote [multiplexer domain](../../../multiplexing.md)
are not waited for.
