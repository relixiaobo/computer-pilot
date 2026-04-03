---
name: computer-pilot
description: >
  Control the macOS desktop via the `cu` CLI tool. Use when the user needs to
  interact with desktop applications — open apps, click buttons, fill forms,
  navigate menus, take screenshots, or read screen content. Works with any
  macOS app via the Accessibility API. Activate this skill whenever a task
  involves desktop automation, app control, GUI interaction, or any operation
  outside the terminal and web browser.
---

# Computer Pilot

Control macOS desktop applications via bash commands. Uses the Accessibility
API for precise element targeting — no coordinate guessing needed.

## Prerequisites

- `cu` binary must be installed (Rust, single binary, zero dependencies)
- Run `cu setup` to check Accessibility + Screen Recording permissions

## Core Workflow: Observe → Act → Verify

```bash
cu apps                              # 1. see what's running
cu snapshot "App Name" --limit 30    # 2. get UI elements with [ref] numbers
cu click 3 --app "App Name"          # 3. interact by ref
cu snapshot "App Name" --limit 30    # 4. verify result
```

## Understanding Snapshots

`cu snapshot` returns the AX tree — a structured list of interactive UI elements:

```
[1] button "Back" (10,40 30x24)
[2] textfield "Search" (100,40 200x24)
[3] statictext "Favorites" (10,100 80x16)
[4] row "Documents" (10,120 300x20)
```

Each element has: `[ref]` number, role, title/value, position, size.
Use the `[ref]` number with `cu click <ref>` to interact.
**Refs change after every action** — always re-snapshot before clicking.

## Commands

### Observe
| Command | Description |
|---------|-------------|
| `cu snapshot [app] --limit N` | AX tree with [ref] numbers (cheapest) |
| `cu ocr [app]` | Vision OCR text recognition (for non-AX apps) |
| `cu screenshot [app] --path file.png` | Window capture (for visual analysis) |
| `cu wait --text "X" --app Name --timeout 10` | Poll until text/element appears |
| `cu apps` | List running applications |

### Act
| Command | Description |
|---------|-------------|
| `cu click <ref> --app Name` | Click element (AX action first, CGEvent fallback) |
| `cu click <ref> --right` | Right-click |
| `cu click <ref> --double-click` | Double-click (open files, select words) |
| `cu click <ref> --shift` | Shift+click (extend selection) |
| `cu click <x> <y>` | Click screen coordinates |
| `cu key <combo> --app Name` | Keyboard shortcut |
| `cu type "text" --app Name` | Type text (Unicode supported) |
| `cu scroll down 5 --x 400 --y 300` | Scroll |
| `cu hover <x> <y>` | Move mouse (tooltips) |
| `cu drag <x1> <y1> <x2> <y2>` | Drag |

### System
| Command | Description |
|---------|-------------|
| `cu setup` | Check permissions + version |

## Perception Strategy

Use the cheapest observation method first:

1. **`cu snapshot`** — structured AX tree (lowest tokens, most precise)
2. **`cu ocr`** — Vision OCR (for apps with poor AX: games, Qt, Java)
3. **`cu screenshot`** — image file (use your own vision to analyze)

## macOS Operation Patterns

### Open an app
```bash
cu key cmd+space                    # open Spotlight
cu type "Calculator" --app Spotlight  # search
cu key enter --app Spotlight         # launch
cu wait --text "Calculator" --timeout 5  # wait for it
```

### Navigate menu bar
```bash
# Method 1: Help menu search (works in most apps)
cu key cmd+shift+/ --app "App Name"
cu type "menu item name" --app "App Name"
cu key enter --app "App Name"

# Method 2: Direct menu shortcut
cu key cmd+, --app "App Name"      # Preferences/Settings
cu key cmd+n --app "App Name"      # New
cu key cmd+o --app "App Name"      # Open
cu key cmd+s --app "App Name"      # Save
cu key cmd+w --app "App Name"      # Close window
cu key cmd+q --app "App Name"      # Quit app
```

### About window (get app version)
```bash
# Click app name in menu bar, then About
cu snapshot "App Name" --limit 50   # find menu bar items
cu click <menu-ref> --app "App Name"
# Or use keyboard: most apps respond to cmd+shift+/ → "About"
```

### Text selection and clipboard
```bash
cu key cmd+a --app "App Name"       # select all
cu key cmd+c --app "App Name"       # copy
pbpaste                              # read clipboard
echo "text" | pbcopy                 # write clipboard
cu key cmd+v --app "App Name"       # paste
```

### File operations in Finder
```bash
cu snapshot Finder --limit 50        # see files
cu click <file-ref> --app Finder     # select file
cu click <file-ref> --double-click --app Finder  # open file
cu key cmd+delete --app Finder       # move to trash
cu key cmd+shift+n --app Finder      # new folder
```

### Handle dialogs and alerts
```bash
cu snapshot --limit 30               # frontmost app (dialog is usually frontmost)
# Look for button refs like "OK", "Cancel", "Allow", "Save"
cu click <button-ref>
```

## Key Rules

- **Always use `--app`** to target a specific app. Without it, keys go to the frontmost app which may have changed.
- **Refs are ephemeral** — they change after every UI mutation. Always re-snapshot.
- **Observe before acting** — don't guess refs from memory. Run `cu snapshot` first.
- **Verify after acting** — `cu snapshot` again to confirm the action worked.
- **JSON output** (when piped) auto-includes a fresh snapshot after click/key/type. Use `--no-snapshot` to disable.

## Output Format

When piped (default for agents), output is JSON:
```json
{"ok":true,"app":"Finder","window":"Downloads","elements":[{"ref":1,"role":"button","title":"Back","x":10,"y":40,"width":30,"height":24}]}
```

Errors include context:
```json
{"ok":false,"error":"element [99] not found in AX tree (scanned 50 elements)"}
```

## Tips

- For Chrome/web tasks, prefer `bp` (browser-pilot) over `cu` — DOM-level precision beats AX tree for web content
- For desktop app tasks, `cu` is the right tool — AX tree provides element refs that Chrome CDP can't access
- If `cu snapshot` returns sparse results, try `cu ocr` (Vision OCR) or `cu screenshot` (visual fallback)
- Use `cu wait --text "X"` after actions that trigger loading or transitions
- Clipboard: use `pbcopy`/`pbpaste` directly, no need for `cu` wrapper
