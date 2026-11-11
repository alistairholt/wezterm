---
tags:
  - gpu
---
# `webgpu_preferred_adapter`

{{since('20221119-145034-49b9839f')}}

Specifies which WebGpu adapter should be used.

On this macOS-only fork wezterm always renders with Metal, so only Metal
adapters are listed and used.

You can use the [wezterm.gui.enumerate_gpus()](../wezterm.gui/enumerate_gpus.md) function
to return a list of GPUs.

If you open the [Debug Overlay](../keyassignment/ShowDebugOverlay.md) (default:
<kbd>CTRL</kbd> + <kbd>SHIFT</kbd> + <kbd>L</kbd>) you can interactively review
the list:

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

Based on that list, I might choose to explicitly target that GPU like
this (but note that this would be the default selection anyway):

```lua
config.webgpu_preferred_adapter = {
  backend = 'Metal',
  device = 0,
  device_type = 'IntegratedGpu',
  name = 'Apple M2 Pro',
  vendor = 0,
}
```

alternatively, I might use:

```lua
local wezterm = require 'wezterm'
local config = {}
local gpus = wezterm.gui.enumerate_gpus()

config.webgpu_preferred_adapter = gpus[1]
return config
```

If you have a more complex situation you can get a bit more elaborate; this
example picks an integrated GPU when one is available:

```lua
local wezterm = require 'wezterm'
local config = {}

for _, gpu in ipairs(wezterm.gui.enumerate_gpus()) do
  if gpu.device_type == 'IntegratedGpu' then
    config.webgpu_preferred_adapter = gpu
    break
  end
end

return config
```

See also [webgpu_power_preference](webgpu_power_preference.md),
[webgpu_force_fallback_adapter](webgpu_force_fallback_adapter.md).
