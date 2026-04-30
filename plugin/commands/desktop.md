---
name: desktop
description: Control the macOS desktop — open apps, click buttons, type text, take screenshots
user-invocable: true
---

Use the `cu` CLI tool to control the macOS desktop for $ARGUMENTS.

Steps:
1. **Start with `cu state <app>`.** One call returns snapshot + windows + screenshot + frontmost — replaces three separate round-trips. Reserve `cu setup` for when permissions are actually broken; reserve `cu apps` for when you don't know what's running.
2. **App not yet running?** `cu launch <name|bundleId>` waits until the first AX-ready window (avoids the empty-tree problem on cold starts). Don't open apps via `cu key cmd+space` — `cu key`/`cu type` are refused when frontmost is a terminal/IDE, and the agent runs from a terminal.
3. **Scriptable apps** (marked `S` in `cu apps`): prefer the scripting workflow:
   - `cu sdef <app>` to discover available commands and classes
   - `cu tell <app> '<AppleScript>'` to read/write app data directly
   - Faster, more reliable, and cheaper than UI automation
4. **Non-scriptable apps**: walk the AX tree:
   - `cu menu <app>` to enumerate the menu bar (works for ANY app)
   - `cu find --app X --role R --title-contains S --first --raw` for a known target — faster than `snapshot + grep`
   - `cu set-value <ref> "..."` for textfields (no focus, no IME)
   - `cu click <ref> --app X` — verify is on by default; read `verified` + `verify_advice`
   - `cu click --text "Label" --app X` (OCR) when AX is sparse
5. **Multi-step flows**: capture `axPath` from the first snapshot and pass `--ax-path` to subsequent calls. Refs renumber across snapshots; axPaths survive.
6. **System settings**: `cu defaults read/write <domain> <key>` skips the System Settings UI entirely.
7. **Window operations**: `cu window list/move/resize/focus/minimize/close`.
8. **Diagnose failures**: when `verified=false` or an action fails, run `cu why <ref> --app X` for a structured "what's wrong with this ref" report.

Always pass `--app "AppName"` on every action command. Without `--app`, events go through the global HID tap and may land on whatever shifted focus.

Read every `*_hint` / `*_reason` / `*_advice` / `*_error` string in the response — when present, the result is degraded or auto-corrected and the agent must react.
