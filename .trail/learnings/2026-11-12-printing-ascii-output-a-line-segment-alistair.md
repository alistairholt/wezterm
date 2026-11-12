---
title: "Printing ASCII output a line segment at a time, and how to measure terminal throughput and memory"
author: Alistair
date: 2026-11-12
branch: claude/memory-throughput
cost: { minutes: 19, tokens: 428623 }
id: l-c72862cc
scope: repo
status: active
src: agent
touches:
  - term/examples/term_throughput.rs
  - term/src/screen.rs
  - term/src/terminalstate/performer.rs
  - term/src/test/mod.rs
  - wezterm-surface/src/line/clusterline.rs
  - wezterm-surface/src/line/line.rs
  - wezterm-toast-notification/src/macos.rs
  - docs/changelog.md
---

## Decided
- The terminal model wrote output one grapheme at a time (segmentation, width lookup, pen clone, attribute compare per cell). Runs of printable ASCII now go straight onto the line in one append when no charset remap, insert mode or no-autowrap is active. Headless 2x for plain text, 2.7x for long lines, 1.3x for colored output; GUI cat 1.7x / 2.3x / 1.2x.
- The last ASCII byte before a non-ASCII character takes the general path, because a following combining mark joins it into one grapheme.
- Lines moving into scrollback shrink their text and cluster vectors (ClusteredLine::new reserves 80 bytes): 22-34% less heap per scrollback line.

## Tried and ruled out
- Treating GUI footprint as a measure of scrollback size: after one cat the GUI grows ~50 MB even with 3,500 lines of scrollback. About 18 MB is Metal drawables (all three get allocated once painting is continuous) and ~19 MB is the 1024-entry line quad and line shape caches holding lines that scrolled away. Scrollback itself is 1-3 MB there.

## Watch out
- Feeding a file straight into Terminal without a pty leaves \n as a bare line feed, so the cursor never returns and every line gets padded. Translate \n to \r\n (the bench example does) or numbers are wrong.
- Grepping cargo output for "^(error|warning)" with head can hide a failed build under deprecation warnings; a stale binary then gets measured. Check the exit status.
- A bare wezterm-gui binary aborts the first time it shows a notification (UNUserNotificationCenter needs a .app); the embedded Info.plist means bundleIdentifier is not a usable check.

In the code:
- term/src/terminalstate/performer.rs flush_print, print_ascii_run, can_print_ascii_runs; test ascii_fast_path_matches_general_path in term/src/test/mod.rs
- wezterm-surface/src/line/line.rs set_cells_ascii, compress_for_scrollback; clusterline.rs append_ascii, shrink_to_fit
- cargo run --release -p wezterm-term --example term_throughput -- [--scrollback N] FILE...
- GUI memory breakdown: vmmap --summary PID (IOSurface vs MALLOC), MallocStackLogging=1 + malloc_history PID -allBySize
