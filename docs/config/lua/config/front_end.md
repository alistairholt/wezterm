---
tags:
  - gpu
---
# `front_end = "WebGpu"`

Specifies which render front-end to use.  This option used to have
more scope in earlier versions of wezterm, but today it allows three
possible values:

* `OpenGL` - use GPU accelerated rasterization
* `Software` - use CPU-based rasterization.
* `WebGpu` - use GPU accelerated rasterization {{since('20221119-145034-49b9839f', inline=True)}}

{{since('20240127-113634-bbcac864', outline=true)}}
    The default is `"WebGpu"`. In earlier versions it was `"OpenGL"`

{{since('20240128-202157-1e552d76', outline=true)}}
    The default has been reverted to `"OpenGL"`.

{{since('nightly', outline=true)}}
    This macOS-only fork always renders with Metal (`"WebGpu"`). The
    OpenGL and Software renderers have been removed. The option is still
    accepted so that existing configs load, but its value is ignored.

In upstream wezterm you may wish (or need!) to select `Software` if there
are issues with your GPU/OpenGL drivers. That choice doesn't exist on this
fork.

## WebGpu

{{since('20221119-145034-49b9839f')}}

The WebGpu front end allows wezterm to use GPU acceleration provided by
a number of platform-specific backends:

* Metal (on macOS)
* Vulkan
* DirectX 12 (on Windows)

On this macOS-only fork, WebGpu always uses Metal, and wezterm presents
each frame into the window's Core Animation layer itself. During a live
window resize every step is drawn at the new size and shown together with
the new window frame, so text is not stretched while you drag. If a step
can't be drawn in time, the previous frame stays at its normal size,
pinned to the top-left corner.

See also:

* [webgpu_preferred_adapter](webgpu_preferred_adapter.md)
* [webgpu_power_preference](webgpu_power_preference.md)
* [webgpu_force_fallback_adapter](webgpu_force_fallback_adapter.md)
