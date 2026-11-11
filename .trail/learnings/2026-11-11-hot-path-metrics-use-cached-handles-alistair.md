---
title: Hot-path metrics use cached handles and the stats thread closes rate windows
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 10, tokens: 90439 }
id: l-80f324aa
scope: repo
status: active
src: agent
touches:
  - docs/changelog.md
  - wezterm-gui/src/quad.rs
  - wezterm-gui/src/stats.rs
  - lfucache/src/lib.rs
---

## Decided
- `metrics::histogram!("name")` looks the handle up in the recorder on every call: a global mutex plus a map search. LfuCache now keeps its hit and miss handles. GUI code that runs per glyph, cluster or line uses `crate::stats::cached_histogram!("name")`, which looks the handle up once per call site.
- `*.rate` metrics (Throughput in stats.rs) used to read the clock on every record to see whether their one-second window had ended. Record now only adds to a count. The stats thread, which wakes every second anyway, closes the windows (`roll`).

## Measured
Release build, Time Profiler, 200x60, same frame counts (about 900 frames in 15s):
- Metrics on the main thread: 12.9 → 0.7 ms/s while scrolling, 1.9 → 0.2 ms/s on a full redraw.
- Main thread CPU while scrolling: 215 → 201 ms/s.

## Watch out
- A `cached_histogram!` or LfuCache created before `Stats::init` gets a no-op handle forever. All current users run after init in wezterm-gui's main.
- The stat log's "current" column for rates is now the last full second, not the count so far in the open window. Windows with no events aren't recorded.
- Total main-thread CPU between runs is noisy because of Core Animation's transaction commit (about ±20 ms/s). Compare wezterm frames such as paint_impl, and check frame counts in ~/.local/share/wezterm/*log*.txt (count "metal_present: step" lines with WEZTERM_LOG=warn,wezterm_gui::termwindow::metal_present=trace). The GUI logs to that file, not stdout.

In the code: wezterm-gui/src/stats.rs, lfucache/src/lib.rs.
