---
tags:
  - tab_bar
---
# `use_fancy_tab_bar = true`

{{since('20220101-133340-7edc5b5a')}}

When set to `true` (the default), the tab bar is rendered in
a native style with proportional fonts.

When set to `false`, the tab bar is rendered using a retro
aesthetic using the main terminal font.

{{since('nightly')}}

[tab_style](tab_style.md) takes the place of this setting and adds the
Flat and Capsule styles. `use_fancy_tab_bar` still works: it is only read
when `tab_style` isn't set, where `true` means `"Fancy"` and `false` means
`"Retro"`.
