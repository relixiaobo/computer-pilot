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
4. **For non-scriptable apps** or when scripting can't do it: use the AX tree workflow:
   - `cu snapshot [app] --limit 30` to get UI elements with [ref] numbers
   - Use `cu click`, `cu key`, `cu type` to interact with elements
   - Verify results with another `cu snapshot`
5. Fall back to `cu ocr` or `cu screenshot` if the AX tree is sparse.

To open an app: `cu key cmd+space`, then `cu type "AppName"`, then `cu key enter`.
To use menus: `cu key cmd+shift+/` opens Help menu search in most apps.

Always use `--app "AppName"` to target a specific app for reliable key/type delivery.
