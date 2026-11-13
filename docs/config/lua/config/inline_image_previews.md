---
tags:
  - appearance
  - mouse
---
# `inline_image_previews = true`

{{since('nightly')}}

Draws a thumbnail under each path in a pane's output that names an image
file, for example the screenshots that Claude Code lists by path.

A program's output can't be changed to make room for a picture, so the
thumbnail takes rows of its own on screen and the rows below it are drawn
further down. If the text already reaches the bottom of the pane, the
rows above move up instead, as they do when a program prints more
output; scroll back to see them. The program still sees the same rows,
and selecting and clicking text work as before.

Click a thumbnail to open the image at full size in Quick Look. The
other images on screen are loaded too, so the arrow keys step through
them.

A path gets a thumbnail when:

* it ends in `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.bmp`, `.tif` or
  `.tiff`, and has a `/` in it. A bare file name such as `photo.png`
  doesn't count, so `ls` in a folder of pictures doesn't fill the pane;
* the file exists on this machine and is no larger than 100MB. Relative
  paths are looked up in the pane's current directory, and `~/` is your
  home directory.

This also applies to full-screen programs such as Claude Code. Those
can't be scrolled back by the terminal, so scroll inside the program to
see rows that moved off the top.

A path that wraps, or that the program broke over several rows, is put
back together first. Paths with spaces are not found. A file that changes
gets a new thumbnail the next time the pane's content changes.

An image wider than the space from the start of its row's text to the
right edge of the pane is scaled down to fill that space; a narrower
image is drawn at its own size. If the thumbnail is then taller than
[inline_image_preview_max_rows](inline_image_preview_max_rows.md) rows,
only the top of the image is shown, fading out at the bottom; click it
to see all of it. When several thumbnails are on
screen they share half of the pane's rows, so each one is shorter, down
to three rows.

```lua
config.inline_image_previews = false
```
