---
tags:
  - appearance
---
# `inline_image_preview_max_rows = 8`

{{since('nightly')}}

The most rows that a thumbnail drawn by
[inline_image_previews](inline_image_previews.md) takes. An image
wider than the space it has is scaled down to fit; if it is then taller
than this, only the top
of the image is shown.

```lua
config.inline_image_preview_max_rows = 12
```
