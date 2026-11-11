---
title: Time only shape cache misses; timing every hit cost more than the lookup
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 10, tokens: 104713 }
id: l-f8ed5e85
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/quad.rs
  - wezterm-gui/src/termwindow/mod.rs
  - wezterm-gui/src/termwindow/render/pane.rs
  - wezterm-gui/src/shapecache.rs
  - wezterm-gui/src/termwindow/render/mod.rs
---

## Decided
The shape cache lookup in the GUI times only misses. A hit takes about 100 ns. Reading the clock twice and recording a histogram value for every cluster drawn cost more than that: 10.8 ms/s of main-thread time at 200x60 in the scroll test. Timing misses only cut it to 3.5 ms/s, and the lookup function fell from 52 to 43 ms/s. Hits are still counted by the cache's own hit rate.

## Tried and ruled out
A cheaper hash and compare for the shape cache key. The trace could not show a gain: the time I took for hashing was mostly the LFU cache's red-black tree, not the hash. Reverted.

## Watch out
Don't put per-hit timers back on hot cache lookups. Time the slow path instead.

In the code: wezterm-gui/src/termwindow/render/mod.rs, cached_cluster_shape. The histogram record and log::trace! sit after shape_cache.put in the miss branch.
