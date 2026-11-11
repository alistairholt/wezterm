---
title: Finder Services are declared in Info.plist and handled by the app delegate as the services provider
author: Alistair
date: 2026-11-11
branch: claude/code-review-4598b5
cost: { minutes: 2, tokens: 25721 }
id: l-de2753a7
scope: repo
status: active
src: agent
touches:
  - assets/macos/WezTerm.app/Contents/Info.plist
  - window/src/os/macos/app.rs
  - wezterm-gui/src/frontend.rs
---

## Decided
"New WezTerm Window Here" / "New WezTerm Tab Here" are NSServices entries (NSSendFileTypes public.folder, empty NSRequiredContext so they're on by default). The app delegate is set as the services provider in did_finish_launching and implements weztermServiceOpenWindow:userData:error: and weztermServiceOpenTab:userData:error:. Window reuses the OpenCommandScript folder path; tab sends PerformKeyAssignment(SpawnCommandInNewTab{cwd}), and the frontend's app-level handler now puts a NewTab into the last focused window (resolve_focused_pane for the GUI client), because with no source window it would open a new window instead.

## Watch out
- The error out-parameter must be typed *mut c_void: objc's Encode isn't implemented for *mut *mut Object, so add_method won't compile.
- Test without Finder: register a bundled copy with lsregister -f (rename the service titles in its Info.plist), run `/System/Library/CoreServices/pbs -update`, then call NSPerformService(title, pasteboard-with-file-URL) from a small Swift program. Unregister and pbs -update afterwards.

## In the code:
assets/macos/WezTerm.app/Contents/Info.plist NSServices; window/src/os/macos/app.rs service_folders, service_open_window, service_open_tab; wezterm-gui/src/frontend.rs PerformKeyAssignment spawn_command.
