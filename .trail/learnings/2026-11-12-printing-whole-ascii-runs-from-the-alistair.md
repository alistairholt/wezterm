---
title: "Printing whole ASCII runs from the escape parser is opt-in, because termwiz's public parse must keep one Print per character"
author: Alistair
date: 2026-11-12
branch: claude/memory-throughput
cost: { minutes: 22, tokens: 159058 }
id: l-98ab4aaa
scope: repo
status: active
src: agent
touches:
  - wezterm-escape-parser/src/parser/mod.rs
  - vtparse/src/lib.rs
  - wezterm-gui/src/termwindow/metal_present.rs
  - docs/changelog.md
---

## Decided
The escape parser can hand a run of printable ASCII to the terminal as one PrintString instead of one Print per character. This is opt-in through a separate entry point. The plain parse keeps emitting one Print per character. Only the terminal's own hot paths (the mux parser thread, Terminal::advance_bytes and the throughput example) use the run version.

## Tried and ruled out
- Changing parse itself to emit PrintString. termwiz tests failed (render::terminfo::test::bold_text and others), because termwiz is a published library and its callers match on per-character Print actions. Changing parse's output would break them.

## Watch out
- Action::append_to merges PrintString into a previous PrintString or Print. Without that, the run version and parse give different action lists.
- vtparse only takes the run path in the Ground state, for bytes 0x20..=0x7f (DEL prints in Ground). In any other state, including partway through a UTF-8 sequence split across reads, it goes byte by byte.

In the code:
- wezterm-escape-parser/src/parser/mod.rs: Parser::parse_with_print_runs, Performer.print_runs, print_str
- vtparse/src/lib.rs: VTActor::print_str (default calls print per char), the Ground run scan in parse
- Tests: vtparse print_runs_match_byte_at_a_time, wezterm-escape-parser print_runs_merge_to_the_same_actions (both try every two-chunk split)
