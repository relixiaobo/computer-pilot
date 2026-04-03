---
name: desktop
description: Control the macOS desktop — open apps, click buttons, type text, take screenshots
user-invocable: true
---

Use the `cu` CLI tool to control the macOS desktop for $ARGUMENTS.

Steps:
1. Run `cu setup` to check permissions. If not ready, guide the user.
2. Run `cu apps` to see what's running.
3. Run `cu snapshot [app] --limit 30` to get UI elements with [ref] numbers.
4. Use `cu click`, `cu key`, `cu type` to interact with elements.
5. Verify results with another `cu snapshot`.

To open an app: `cu key cmd+space`, then `cu type "AppName"`, then `cu key enter`.
To use menus: `cu key cmd+shift+/` opens Help menu search in most apps.

Always use `--app "AppName"` to target a specific app for reliable key/type delivery.
