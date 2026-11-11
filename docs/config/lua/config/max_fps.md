---
tags:
  - tuning
---
# `max_fps = 120`

Limits the maximum number of frames per second that wezterm will
attempt to draw.

Defaults to `120`. (Upstream wezterm defaults to `60`.)

On macOS, painting follows the display's refresh through the window's
display link, so the frame rate is the lower of `max_fps` and the
refresh rate of the display the window is on. The default of `120`
gives smooth 120Hz output on ProMotion displays and makes no difference
on 60Hz displays. Set `max_fps = 60` to save power on a ProMotion
display. When nothing on screen changes, wezterm doesn't paint at all.

| Environment | Supported Since |
|-------------|-----------------|
| Wayland     | Ignored; instead, uses information from the compositor to schedule painting frames |
| X11         | {{since('20211204-082213-a66c61ee9', inline=True)}} |
| macOS       | {{since('20220903-194523-3bb1ed61', inline=True)}} |
| Windows     | {{since('20220903-194523-3bb1ed61', inline=True)}} |
