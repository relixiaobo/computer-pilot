# Computer Pilot CLI — Complete Command Reference

## Setup

### `cu setup`
Check permissions, version, and guide authorization.
- Reports: Accessibility (required for snapshot/click/key/type), Screen Recording (required for screenshot/OCR), and automation/scripting_ready status
- Opens System Settings if permissions are missing

## Scripting

### `cu tell <app> '<AppleScript>'`
Execute AppleScript against an app. Auto-wraps in `tell application "..." ... end tell`.
- `cu tell "System Events" 'get name of every process whose visible is true'`
- `cu tell Finder 'get name of every item of desktop'`
- Uses `-ss` flag for structured output (unambiguous text)
- Auto-launches the app if not already running
- Timeout enforced (default 10s)

### `cu sdef <app>`
Show the scripting dictionary for an app.
- Returns classes, properties, and commands the app exposes via AppleScript
- `cu sdef Finder` — discover Finder scripting capabilities
- Pure Rust XML parsing (no external tools)
- Use this to discover what operations are available before writing `cu tell` commands

## Observation

### `cu apps`
List running applications with name, PID, active status, scriptable flag, and sdef_classes count.
- `*` = frontmost app, `S` = AppleScript scriptable
- Scriptable apps show `sdef_classes` count indicating scripting dictionary richness

### `cu snapshot [app] [--limit N]`
Get the AX tree — interactive UI elements with `[ref]` numbers.
- `app` — target app name (default: frontmost)
- `--limit 50` — max elements (default: 50)
- Returns: ref, role, title, value, x, y, width, height per element
- Only interactive roles get refs: button, textfield, textarea, statictext, row, cell, checkbox, radiobutton, popupbutton, combobox, link, menuitem, menubutton, tab, slider, image

### `cu screenshot [app] [--path file.png] [--full]`
Capture window screenshot silently. No app activation needed.
- `--path /tmp/shot.png` — output path (default: auto-generated in /tmp)
- `--full` — capture entire screen (all monitors) instead of single window
- Window mode returns `offset_x`, `offset_y` for coordinate translation: `screen = pixel + offset`

### `cu ocr [app]`
Recognize text on screen via macOS Vision framework.
- Returns text with screen coordinates and confidence scores
- Best for apps with poor AX support (games, Qt, Java)
- Runs on-device, no network needed

### `cu wait --text "X" | --ref N | --gone N [--app name] [--timeout 10] [--limit 200]`
Poll the AX tree until a condition is met.
- `--text "Submit"` — wait until any element contains this text
- `--ref 5` — wait until element ref 5 exists
- `--gone 5` — wait until element ref 5 disappears
- `--timeout 10` — seconds before giving up (default: 10)
- Note: prefer `--text` over `--ref`/`--gone` since refs are unstable across UI changes

## Interaction

### `cu click <target> [y] [--app name] [--right] [--double-click] [--shift] [--cmd] [--alt] [--no-snapshot]`
Click an element by ref or screen coordinates.
- `cu click 3 --app Finder` — click ref [3] from snapshot
- `cu click 500 300` — click screen coordinates
- `--right` — right-click
- `--double-click` — double-click (open files, select words)
- `--shift` — shift+click (extend selection)
- `--cmd` — cmd+click (toggle selection, open in new tab)
- `--alt` — alt/option+click
- Ref mode: tries AX actions (AXPress/AXConfirm) first, falls back to CGEvent
- Right-click and double-click always use CGEvent (skip AX actions)

### `cu key <combo> [--app name] [--no-snapshot]`
Send a keyboard shortcut.
- `cu key cmd+c --app "Google Chrome"` — copy
- `cu key cmd+shift+n --app "Google Chrome"` — new incognito window
- `cu key cmd+space` — open Spotlight
- `cu key enter --app Safari` — confirm
- `cu key escape` — cancel/close
- Modifiers: `cmd`, `shift`, `ctrl`, `alt` (option)
- Keys: a-z, 0-9, enter, tab, space, escape, delete, up/down/left/right, f1-f12, minus, equal, etc.
- With `--app`: uses AppleScript System Events (reliable delivery to target app)
- Without `--app`: uses CGEvent (sends to frontmost app)

### `cu type <text> [--app name] [--no-snapshot]`
Type text into the focused element via clipboard paste. Safe with any IME. Unicode supported.
- `cu type "hello world" --app TextEdit`
- `cu type "https://example.com" --app "Google Chrome"`
- Uses clipboard paste (Cmd+V) for reliable input regardless of keyboard layout or IME
- With `--app`: activates app first, then pastes

### `cu scroll <direction> <amount> --x X --y Y`
Scroll at specified coordinates.
- Directions: `up`, `down`, `left`, `right`
- Amount: number of lines (default: 3)
- `cu scroll down 5 --x 400 --y 300`

### `cu hover <x> <y>`
Move mouse to coordinates. Triggers tooltips and hover menus.

### `cu drag <x1> <y1> <x2> <y2> [--shift] [--cmd] [--alt]`
Drag from one position to another with smooth 10-step interpolation.
- `cu drag 100 200 400 200` — drag right
- `--shift` — shift+drag (extend selection)
- `--alt` — option+drag (copy in Finder)
- Guarantees mouseUp even if intermediate steps fail

## Output

JSON when piped (default for agents), human-readable when TTY.

Action commands (`click`, `key`, `type`) include a fresh snapshot in JSON mode:
```json
{"ok":true,"combo":"cmd+t","snapshot":{"ok":true,"app":"Chrome","elements":[...]}}
```

Use `--no-snapshot` to suppress the auto-snapshot.

Use `--human` flag to force human-readable output regardless of pipe status.
