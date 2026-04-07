---
name: desktop
description: Control the macOS desktop — open apps, click buttons, type text, take screenshots
user-invocable: true
---

Use the `cu` CLI tool to control the macOS desktop for $ARGUMENTS.

Steps:
1. Run `cu setup` to check permissions. If not ready, guide the user.
2. Run `cu apps` to see what's running. Apps marked `S` are scriptable.
3. **For scriptable apps** (marked `S`): prefer the scripting workflow:
   - `cu sdef <app>` to discover available commands and classes
   - `cu tell <app> '<AppleScript>'` to execute actions directly
   - Scripting is faster, more reliable, and cheaper than UI automation
4. **For non-scriptable apps**: discover capabilities first, then act:
   - `cu menu <app>` to enumerate the menu bar (Edit, View, Tools, etc.)
   - `cu snapshot [app] --limit 30` for clickable UI elements
   - `cu click <ref>`, `cu key`, `cu type` to interact
   - `cu click --text "Label"` when AX tree is sparse (uses OCR)
5. **For system settings**: skip the System Settings UI:
   - `cu defaults read/write <domain> <key> [value]` for any preference
   - `cu tell "System Events" '...'` for things like dark mode, volume
6. **For window operations**: use `cu window list/move/resize/focus/minimize/close`.
7. Fall back to `cu ocr` or `cu screenshot` if the AX tree is sparse.

To open an app: `cu key cmd+space`, then `cu type "AppName"`, then `cu key enter`.

Always use `--app "AppName"` to target a specific app for reliable key/type delivery.
