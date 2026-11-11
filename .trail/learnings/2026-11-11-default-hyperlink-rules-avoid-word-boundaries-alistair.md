---
title: Default hyperlink rules avoid word boundaries and lookahead so they run in the fast regex engine
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 10, tokens: 74597 }
id: l-401f53c2
scope: repo
status: active
src: agent
touches:
  - wezterm-gui/src/renderstate.rs
  - wezterm-gui/src/termwindow/render/draw.rs
  - wezterm-gui/src/termwindow/webgpu.rs
  - config/src/config.rs
  - docs/config/lua/config/hyperlink_rules.md
  - docs/changelog.md
---

## Decided
fancy-regex (0.14, and still 0.19) treats any `\b` as "hard", not only lookarounds, and runs those rules in its backtracking VM from every start position: about 4-5 µs per line per rule versus about 40 ns in the regex crate. Three upstream default rules used `\b`, costing 0.5-0.9 ms per frame while output scrolled. config's default_hyperlink_rules now uses rewrites that match the same text: drop the leading `\b` (the leftmost match always starts at a word-run start), turn the closing-parenthesis lookahead into a match outside capture group 1 (highlight 1, format $1), and write the mailto trailing `[\w-]+\b` as `(?:\.[\w-]+)*\.[\w-]*\w`. A test compares them with upstream's on fixed and 200,000 random lines.

## Tried and ruled out
- Only removing the lookahead (keeping `\b`): no faster, the rule stayed in the fancy VM.
- Upgrading fancy-regex: 0.19 still treats `\b` as hard.

## Watch out
- Any new default or suggested rule with `\b`, lookaround or backrefs goes back to the slow engine.
- wezterm-surface's CLOSING_PARENTHESIS_HYPERLINK_PATTERN and GENERIC_HYPERLINK_PATTERN are left unchanged (that crate stays upstream-compatible); config no longer uses them.

In the code:
- config/src/config.rs default_hyperlink_rules, test default_rules_match_upstream_rules
- docs/config/lua/config/hyperlink_rules.md
