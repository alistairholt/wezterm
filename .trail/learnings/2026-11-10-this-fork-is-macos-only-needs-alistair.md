---
title: "This fork is macOS-only, needs macOS 14, and no longer follows upstream"
author: Alistair
date: 2026-11-10
branch: main
cost: { minutes: 2, tokens: 11888 }
id: l-880290c1
scope: repo
status: active
src: agent
touches:
  - ".cargo/config.toml"
  - assets/macos/WezTerm.app/Contents/Info.plist
  - ci/generate-workflows.py
---

## Decided
- The fork targets macOS only and does not track upstream wezterm. Upstream activity is low (no tagged release since Feb 2024) and about a third of it is Windows/Linux/packaging work. Platform-neutral fixes in the terminal model and escape parser can still be cherry-picked, so those crates are kept unchanged and platform-neutral.
- Minimum macOS is 14, so NSView.displayLink can drive 120 Hz frame pacing.
- Rendering moves to Metal by keeping wgpu but having WezTerm own the CAMetalLayer drawables, presenting each live-resize step inside AppKit's display pass. wgpu resets presentsWithTransaction on every configure, so toggling it through wgpu doesn't work. CGL/EGL/glium get removed after the Metal path is the default.

## Tried and ruled out
- A native Metal encoder without wgpu: 2-3 weeks for no measured gain. Revisit only if profiling shows wgpu overhead.

## Watch out
- Don't reintroduce non-macOS cfg branches or dependencies.
- Colour-managed sRGB output changes how themes look on P3 screens; it was left as an open taste decision (planned as a config option).

In the code:
- .cargo/config.toml [env] MACOSX_DEPLOYMENT_TARGET = "14.0"; ci/generate-workflows.py; LSMinimumSystemVersion in assets/macos/WezTerm.app/Contents/Info.plist
- Keep unchanged: term, termwiz, wezterm-escape-parser, vtparse, wezterm-cell, wezterm-surface
- Metal plan: docs/internal/metal-renderer-plan.md on branch metal-design
