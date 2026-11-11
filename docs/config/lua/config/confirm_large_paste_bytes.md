---
tags:
  - clipboard
---
# `confirm_large_paste_bytes = 0`

{{since('nightly')}}

When set above `0`, WezTerm asks before pasting more than this many
bytes, as kitty's `paste_actions = confirm-if-large` does. A large
paste is often a mistake, such as copying a whole log file, and can
take a program a long time to read. It is off by default.

```lua
-- Ask before pasting more than 16 KB
config.confirm_large_paste_bytes = 16 * 1024
```

The question appears in the pane, with Yes and No buttons. Nothing is
pasted unless you choose Yes.

See also [confirm_multiline_paste](confirm_multiline_paste.md).
