---
name: computer-pilot
description: >
  Control the macOS desktop via the `cu` CLI tool. Use when the user needs to
  interact with desktop applications — open apps, read/write app data, click
  buttons, fill forms, navigate menus, take screenshots, or read screen content.
  Three-tier control: AppleScript (scriptable apps) → AX tree + CGEvent
  (non-scriptable) → OCR + screenshot (fallback). Activate this skill whenever
  a task involves desktop automation, app control, GUI interaction, or any
  operation outside the terminal and web browser.
---

# Computer Pilot

Control macOS desktop applications via the `cu` CLI. Three tiers of control:
1. **AppleScript** (`cu tell`) — direct app data access for scriptable apps (fastest, most reliable)
2. **AX tree** (`cu snapshot` + `cu click`) — UI element automation for any app
3. **OCR/Screenshot** — visual fallback for apps with poor AX support

## Prerequisites

- `cu` binary must be installed (Rust, single binary)
- Run `cu setup` to check Accessibility + Screen Recording + Automation permissions

## Choose the Right Approach

```bash
cu apps                              # S flag = scriptable → use cu tell
```

**Scriptable app (has S flag)?** → Use `cu tell` with AppleScript. Faster, more reliable, reads/writes app data directly.

**Non-scriptable app?** → Use `cu menu` to discover capabilities, then `cu snapshot` → `cu click` for interaction.

**System settings?** → Use `cu defaults read/write` to change preferences directly (no System Settings UI needed).

## Scripting Workflow (preferred for scriptable apps)

```bash
cu apps                                          # 1. check S flag
cu sdef "App Name"                               # 2. discover available properties/commands
cu tell "App Name" 'get name of every calendar'  # 3. read/write data via AppleScript
```

### Examples

```bash
# Read data
cu tell Safari 'get URL of current tab of front window'
cu tell Finder 'get name of every item of front Finder window'
cu tell Notes 'get plaintext of note 1'
cu tell Reminders 'get name of every reminder whose completed is false'
cu tell "System Events" 'get dark mode of appearance preferences'

# Mail — read emails (use specific mailbox for speed, "inbox" is slow on large mailboxes)
cu tell Mail 'get subject of message 1 of inbox'
cu tell Mail 'get content of message 1 of inbox'                        # email body text
cu tell Mail 'get {subject, sender, date received} of messages 1 thru 5 of inbox'
cu tell Mail 'get content of message 1 of mailbox "INBOX" of account 1' # faster on large mailboxes

# Calendar — multi-line scripts work (passed via stdin)
cu tell Calendar 'set d to (current date) + (1 * days)
set hours of d to 10
set minutes of d to 0
set seconds of d to 0
make new event at end of events of first calendar with properties {summary:"Meeting", start date:d, end date:d + (1 * hours)}'

# Write data
cu tell Notes 'make new note with properties {name:"Title", body:"Content"}'
cu tell Reminders 'make new reminder with properties {name:"Buy milk"}'

# System control
cu tell "System Events" 'set dark mode of appearance preferences to true'
```

## AX Tree Workflow (for non-scriptable apps)

For apps WITHOUT the `S` flag in `cu apps`: go straight to UI automation. Do NOT try `cu tell` — it will waste steps.

```bash
cu snapshot "App Name" --limit 50    # 1. get UI elements with [ref] numbers
cu click 3 --app "App Name"          # 2. interact by ref
cu snapshot "App Name" --limit 50    # 3. verify result, get new refs
cu click 7 --app "App Name"          # 4. next action with updated refs
```

### Example: Calculator (not scriptable)
```bash
cu snapshot Calculator --limit 30    # See: [6] button "All Clear", [18] button "1", etc.
cu click 6 --app Calculator          # Clear
cu click 18 --app Calculator         # Press "1"
cu click 16 --app Calculator         # Press "6"
cu click 22 --app Calculator         # Press "0"
cu click 13 --app Calculator         # Press "Multiply"
cu click 12 --app Calculator         # Press "9"
cu click 24 --app Calculator         # Press "Equals"
# For scientific calculator: View menu → Scientific (or cmd+2)
cu key cmd+2 --app Calculator        # Switch to scientific mode
cu snapshot Calculator --limit 50    # Now shows sin, cos, tan, etc.
```

### Text-based click (for UI not in AX tree)
When elements don't appear in `cu snapshot`, use OCR text click:
```bash
cu click --text "Edit Widgets" --app "System Settings"    # finds text via OCR, clicks it
cu click --text "Submit" --app Safari --index 2            # click 2nd match
```

## Common Patterns

### Launch any app
```bash
cu key cmd+space                     # Spotlight
cu type "App Name"                   # search
cu key enter                         # launch
cu wait --text "AppName" --timeout 5 # wait for it
```

### Discover any app's capabilities
```bash
cu menu "App Name"                   # list ALL menu items (works for every app)
# Output: View > Scientific, Edit > Copy, File > Export, etc.
# Then click any menu item:
cu tell "System Events" 'tell process "App Name" to click menu item "Scientific" of menu "View" of menu bar 1'
```

### Change system settings (no UI needed)
```bash
cu defaults read com.apple.dock autohide                 # read a preference
cu defaults write com.apple.dock autohide -bool true     # change it
# Common domains: com.apple.dock, com.apple.finder, NSGlobalDomain
# For settings not in defaults, use cu tell "System Events":
cu tell "System Events" 'tell appearance preferences to set dark mode to true'
osascript -e 'set volume output volume 50'               # volume (0-100)
```

### Click any menu item (universal)
```bash
# System Events can click menu items in ANY app
cu tell "System Events" 'tell process "AppName"
  click menu item "About AppName" of menu "AppName" of menu bar 1
end tell'
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

### Discovery (use first to understand what's available)
| Command | Description |
|---------|-------------|
| `cu apps` | List running apps (`S` = scriptable, with class count) |
| `cu menu <app>` | List ALL menu bar items of any app (via System Events) |
| `cu sdef <app>` | Show scripting dictionary for scriptable apps |

### Scripting (for scriptable apps — check S flag in `cu apps`)
| Command | Description |
|---------|-------------|
| `cu tell <app> '<AppleScript>'` | Run AppleScript against app (read/write data) |

### System Preferences (bypass System Settings UI)
| Command | Description |
|---------|-------------|
| `cu defaults read <domain> [key]` | Read a macOS preference |
| `cu defaults write <domain> <key> <value>` | Write a macOS preference |

### Observe
| Command | Description |
|---------|-------------|
| `cu apps` | List running apps (`S` = scriptable, with class count) |
| `cu snapshot [app] --limit N` | AX tree with [ref] numbers |
| `cu ocr [app]` | Vision OCR text recognition (for non-AX apps) |
| `cu screenshot [app] --path file.png` | Window capture (for visual analysis) |
| `cu wait --text "X" --app Name --timeout 10` | Poll until text/element appears |

### Act
| Command | Description |
|---------|-------------|
| `cu click <ref> --app Name` | Click element (AX action first, CGEvent fallback) |
| `cu click <ref> --right` | Right-click |
| `cu click <ref> --double-click` | Double-click (open files, select words) |
| `cu click <ref> --shift` | Shift+click (extend selection) |
| `cu click <x> <y>` | Click screen coordinates |
| `cu key <combo> --app Name` | Keyboard shortcut |
| `cu type "text" --app Name` | Type text (clipboard paste, safe with any IME) |
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
