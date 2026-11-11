# `wezterm.gui.enumerate_gpus()`

{{since('20221119-145034-49b9839f')}}

Returns the list of available Gpus supported by WebGpu.

{{since('nightly', outline=true)}}
    On this macOS-only fork only Metal adapters are listed.

This is useful in conjunction with [webgpu_preferred_adapter](../config/webgpu_preferred_adapter.md)

```
> wezterm.gui.enumerate_gpus()
[
    {
        "backend": "Metal",
        "device": 0,
        "device_type": "IntegratedGpu",
        "name": "Apple M2 Pro",
        "vendor": 0,
    },
]
```
