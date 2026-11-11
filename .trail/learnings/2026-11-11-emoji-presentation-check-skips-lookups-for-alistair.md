---
title: Emoji presentation check skips lookups for single-byte cells
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 9, tokens: 119234 }
id: l-2e49d06b
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - docs/changelog.md
  - docs/config/lua/config/max_fps.md
  - docs/internal/metal-renderer-plan.md
  - window/src/os/macos/window.rs
  - docs/config/lua/config/animation_fps.md
  - docs/config/lua/config/macos_live_resize_repaint_wait_ms.md
  - docs/config/lua/config/webgpu_force_fallback_adapter.md
  - docs/config/lua/config/webgpu_power_preference.md
  - docs/config/lua/config/webgpu_preferred_adapter.md
---

## Decided
Presentation::for_grapheme returns (Text, None) straight away when the grapheme is one byte long. One byte means ASCII; no ASCII character has emoji presentation and every variation sequence ends in a multi-byte selector, so the phf variation-map lookup and the emoji table lookup can't change the answer. It runs for every cell when lines are clustered for drawing (CellCluster::make_cluster -> Cell::presentation).

Measured (Time Profiler, 15 s, 200x60, max_fps 120, same frame counts):
- redraw: wezterm_char_props 187 -> 30 samples; Line::cluster 488 -> 311 (about 10 ms/s of main thread CPU).
- scroll: wezterm_char_props 74 -> 11 samples.

## Watch out
If you add another fast path (for example, caching presentation on the cell), keep the test that compares the fast path with the full lookup for all 128 ASCII characters.

In the code: wezterm-char-props/src/emoji.rs (for_grapheme, for_grapheme_lookup, tests ascii_fast_path_matches_lookup and non_ascii_still_looked_up).
