---
title: "Line shape hashes are computed in the GUI with a four-lane hasher, not SipHash"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 11, tokens: 183702 }
id: l-65eb1b90
scope: repo
status: active
src: agent
touches:
  - config/src/config.rs
  - docs/changelog.md
  - docs/config/lua/config/hyperlink_rules.md
  - wezterm-gui/src/termwindow/render/shape_hash.rs
  - wezterm-gui/src/termwindow/render/mod.rs
  - wezterm-surface/src/line/line.rs
---

## Decided
The GUI hashes each changed line with its own hasher (`line_shape_hash`) instead of `Line::compute_shape_hash`. wezterm-surface is kept upstream-compatible, so the faster hasher lives in wezterm-gui and drives the public per-cell `CellRef::compute_shape_hash` plus the line's public display getters (double width/height, bidi, has_hyperlink).

The hash only keys in-process caches (line state, shape and quad caches), so it doesn't need to match the old value. It uses a random key per process so output can't be crafted to collide.

## Tried and ruled out
- Cost was never cache misses. `shape_hash_for_line` already skips lines whose seqno hasn't changed; the cost was hashing lines that really changed (all 60 rows per frame in a full-screen redraw).
- About 24 tiny `Hasher` writes per cell (text, attribute bits, and each color float as a length-prefixed `[u8; 4]`). Per-write latency dominates, not bytes.
  - SipHash128: 76 ns per cell.
  - One or two ahash hashers: about 33 ns, held back by one long multiply chain.
  - Buffering writes into a 64-byte block and mixing it in four lanes: 45 ns, slower because of the variable-offset copies.
  - Rotating each write across four independent foldhash-style lanes: 10 ns, the one kept.

## Watch out
Real-world gain is about 3× rather than the 7.5× microbenchmark: `shape_hash_for_line` went 31 → 10 ms/s (scroll) and 61 → 19 ms/s (full redraw) at 200x60.

In the code: wezterm-gui/src/termwindow/render/shape_hash.rs; tests compare its grouping of lines with `Line::compute_shape_hash`.
