---
title: "The LFU cache files entries by use count lazily, so a hit doesn't touch the tree"
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 10, tokens: 74718 }
id: l-1d36f413
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - wezterm-surface/src/line/line.rs
  - wezterm-surface/src/line/test.rs
  - lfucache/src/lib.rs
---

## Decided
A hit in the LFU cache only bumps the entry's count. Each entry also keeps the count it is filed under in the frequency tree, which is never above its real count. Eviction takes the lowest filed entry; if its filed count is stale it is filed again under its real count and eviction looks again. The first entry whose filed count is current has the lowest real count, so eviction picks the same minimum as before.

Before, every hit removed the entry from the red-black tree and inserted it again. That was about 6 ms/s of main-thread time while scrolling at 200x60 (frequency tree 6 -> 1.2 ms/s; whole LFU 34 -> 28 ms/s; redraw 11 -> 8.4 ms/s), with frame counts equal (~900 per 15 s).

## Watch out
Within one count, the order of entries in the tree no longer follows hit order, so which of two equal-count entries is evicted can differ from before. The tests' frequency_order helper sorts by (real count, last tick) for that reason.
One eviction can re-file many stale entries at once (up to the cache size) after a long run of hits without evictions. That is bounded and amortised by the hits it replaces.
What is left in the LFU cost is mostly freeing evicted line-shape entries (~5 ms/s), shape key compares (~4 ms/s) and hashing (~3 ms/s).

In the code: lfucache/src/lib.rs, Entry::tree_freq, get, evict_one, decay_least_recent; test eviction_uses_real_counts.
