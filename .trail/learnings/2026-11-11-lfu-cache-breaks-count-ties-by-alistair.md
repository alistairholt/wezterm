---
title: LFU cache breaks count ties by least recent use
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 3, tokens: 23492 }
id: l-066864be
scope: repo
status: active
src: agent
touches:
  - lfucache/src/lib.rs
---

## Decided
The frequency tree is keyed by (filed count, filed last-use tick). Eviction re-files an entry whose count or last use changed since it was filed, so among entries with the lowest count the least recently used is evicted. This restores the order the cache had before hits stopped re-filing entries (the lazy re-filing change): back then a hit moved the entry to the end of its count group.

Found by a review: with keys filed by count alone, two entries filed under 0 and then hit in the order B, A were both re-filed under 1 in filing order (A first), so A, the more recent, was evicted.

## Watch out
- Every filed value must stay at or below the real one (count and tick), or the first "current" entry popped isn't the minimum. Decay lowers the real count, so it re-files with the current tick as well.
- Cost is unchanged: lfucache stayed at 17% of paint_impl samples in the scroll trace.

In the code: lfucache/src/lib.rs (Entry::tree_tick, FrequenceAdapter key, evict_one, decay_least_recent, test equal_counts_evict_least_recent).
