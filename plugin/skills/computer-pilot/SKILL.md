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

## Decision Tree (which command to use)

When you have a goal, walk down this tree before you write any commands:

```
Need to act on the system?
├─ App not yet running?            → cu launch <name|bundleId>      (D6: waits for window-ready)
├─ Starting a task on a specific app?
│                                  → cu state <app>                  (snapshot + screenshot + windows + frontmost in one call)
├─ Read/write app data?
│   ├─ S flag in `cu apps`?        → cu tell <App> '<AppleScript>'
│   └─ otherwise                   → cu snapshot → cu set-value / cu type
├─ Click something?
│   ├─ Know its ref (just snapshotted)? → cu click <ref> --app <X>          (verify is on by default — read `verified` + `verify_advice`)
│   ├─ Multi-step / UI mutates?         → cu click --ax-path "<path>" --app <X>   (A2: stable across snapshots)
│   ├─ Know its text/role?              → cu find --first --raw --role <R> --title-contains <S> --app <X> | xargs cu click --app <X>
│   ├─ Know visual coords (VLM)?        → cu nearest <x> <y> --app <X>   (or cu click <x> <y>)
│   ├─ See it but no AX/text?           → cu click --text "Submit" --app <X>   (OCR-driven)
│   └─ Need extra speed (bulk clicks)?  → cu click <ref> --app <X> --no-verify
├─ Fill a textfield?
│   ├─ AX field, just snapshotted?     → cu set-value <ref> "..."           (no focus needed)
│   ├─ AX field, multi-step flow?      → cu set-value --ax-path "<path>" "..."   (selector survives UI churn)
│   ├─ Chat app or CJK/emoji?          → cu type "..." --app <X>            (auto-routes via paste — see paste_reason in output)
│   └─ Electron / non-AX?              → cu click <ref>; cu type "..."
├─ Read screen?
│   ├─ Tree layout (cheap)?        → cu snapshot --limit N
│   ├─ Only what changed?          → cu snapshot --diff
│   ├─ See a region (VLM)?         → cu screenshot --region "x,y WxH"
│   └─ Pixel + ref labels (VLM)?   → cu snapshot --annotated --output p.png
├─ Wait for state?
│   ├─ Text appears?               → cu wait --text "..." --app <X>
│   ├─ New window opens?           → cu wait --new-window --app <X>
│   ├─ Modal/sheet appears?        → cu wait --modal --app <X>
│   ├─ Focus moves?                → cu wait --focused-changed --app <X>
│   └─ Element disappears?         → cu wait --gone <ref> --app <X>
├─ System preferences?             → cu defaults read/write <domain> <key>
└─ Click/perform returned ok=false (or the UI didn't change)?
                                   → cu why <ref> --app <X>   (B7: structured "what's wrong with this ref" report)
```

**Hard rules:**
- **Every action command needs `--app <Name>`.** Not just `click` — also `cu key`, `cu type`, `cu scroll`, `cu hover`, `cu drag`, `cu set-value`, `cu perform`. Without `--app`, events go through the global HID tap → they hit *whatever's frontmost*. `cu key` and `cu type` now **refuse outright** when frontmost is a terminal/IDE (the most common foot-gun: typing into the terminal that's running cu). Pass `--allow-global` to override, but the right fix is almost always `--app`.
- **Observe the target app directly, even if it's behind other windows.** `cu snapshot <app>` and `cu screenshot --app <app>` both work without bringing the app to the front. Don't run `cu apps`, scan the list, click on a window, then snapshot. Just snapshot the app you care about by name on step 1. **First call when starting a task**: prefer `cu state <app>` — one call returns snapshot + screenshot + window list + frontmost flag, saves a round-trip.
- **Refs are ephemeral, axPaths are stable.** Refs refresh with every snapshot. For multi-step flows that mutate the UI between actions, save the `axPath` from the first snapshot and pass `--ax-path` to subsequent click/set-value/perform calls. The same axPath resolves to the same element even after refs renumber.
- After every action, read the auto-attached `snapshot` (and `settle_ms`, the actual UI-settle wait) instead of calling `cu snapshot` again.
- **`cu type` auto-routes through clipboard paste** when the text contains CJK characters or the target is on the chat-app list (WeChat, Slack, Discord, Telegram, Lark/Feishu, QQ/TIM, DingTalk, WhatsApp, Signal). These apps' CEF-based inputs drop leading characters of `CGEventKeyboardSetUnicodeString` events. When auto-routed, `paste_reason` is in the JSON output. Force off with `--no-paste` if you've confirmed the target handles unicode events. Force on with `--paste`.
- **`cu click` verify is ON by default.** Pre/post AX diff catches sandboxed/Electron apps that silently swallow PID-targeted CGEvents (the #1 "ok=true but UI didn't change" failure). Always read `verified` in the response. When `verified=false`, follow the `verify_advice` string — typical recovery: `cu window focus --app <X>` (non-disruptive ax-raise) then retry, or pass `--allow-global` to use the global tap path (focus shifts but click lands). Pass `--no-verify` only when you've measured the cost (~50–150ms) and need throughput on a known-reliable target.
- **Read the `*_hint` / `*_reason` / `*_advice` strings.** When cu attaches an extra string field beyond the data, it's because the result is degraded or auto-corrected and you need to do something differently. Names to watch: `truncation_hint` (snapshot was clipped — re-run with bigger `--limit`), `confidence_hint` (OCR has low-confidence matches — verify visually before acting), `paste_reason` (type was auto-routed via clipboard), `verify_advice` (action returned ok but didn't move the tree), `screenshot_error` (capture refused — usually `kCGWindowSharingState=0`).
- **`screenshot::find_window` resolves windows via AX**, so `cu screenshot` always captures the same window that `cu snapshot` shows. Multi-window-per-app, off-Space windows, Electron menu-bar stubs — all handled. ScreenCaptureKit is the primary capture path so cross-Space windows produce real PNGs. `kCGWindowSharingState=0` apps (WeChat, some Microsoft Office Mac App Store builds) refuse upfront with a structured error rather than producing a blank PNG.

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

### VLM agents: tree + image in one atomic call

When the VLM needs to look at the actual UI (without ref overlays distracting it) AND act via refs in the same step, use `--with-screenshot`:

```bash
cu snapshot Mail --limit 50 --with-screenshot --output /tmp/m.png
# JSON: elements[] + window_frame + screenshot:/tmp/m.png + image_scale:2.0
```

Both the tree and the image are captured in the same call → guaranteed to reflect the same UI instant (no race between two separate `cu` invocations). Combine with `--diff` to get only changed elements + a fresh image.

If you also want refs drawn on the image, use `--annotated` instead (it supersedes `--with-screenshot`).

### VLM agents: cheap visual verification

To check "did that button turn grey?" or "is the modal gone?", don't re-screenshot the whole window. Use `--region` to grab only the area you care about:

```bash
cu screenshot --region "480,200 400x300" --path /tmp/ck.png  # 200×100 area
# ~150 tokens for the VLM vs ~1500 for a full 1920×1200 window
```

Region coords are in **points** (same space as snapshot element x/y). Output PNG is in pixels (Retina is 2×); response always echoes `offset_x/offset_y/width/height` so you can map back.

### VLM agents: region → candidate refs

When the VLM has narrowed the area of interest to a rectangle (a dialog, a list area, a toolbar), `cu observe-region` returns the candidate set inside it — narrower than `cu snapshot`, more flexible than `cu nearest`:

```bash
cu observe-region 480 200 400 300 --app Mail              # intersect (default)
cu observe-region 480 200 400 300 --app Mail --mode center  # less noise
cu observe-region 480 200 400 300 --app Mail --mode inside  # strictest
```

`--mode` choices:
- `intersect` — element bbox overlaps the rect at all (broadest)
- `center` — element center point falls inside (filters big container noise)
- `inside` — element fully contained (strictest)

Combine with `cu find` filters by piping through jq if you need role/title narrowing on top.

### VLM agents: visual coords → ref

When the VLM has identified WHERE on screen something is but doesn't have a ref, `cu nearest <x> <y>` translates the pixel into the closest interactive element:

```bash
cu nearest 480 320 --app Mail
# → {"match":{"ref":12,"role":"button","title":"Send","distance":0.0,"inside":true}}
REF=$(cu nearest 480 320 --app Mail | jq -r .match.ref)
cu click "$REF" --app Mail
```

`distance:0 inside:true` means the point falls inside the element. With `--max-distance N`, returns `match:null` if nothing's within N points — useful for "did the VLM click on background or a real element" sanity checks.

### VLM agents: look at the screen, click by ref

When the agent has vision, the highest-leverage flow is `--annotated`: cu draws each ref's bounding box + number directly on a window screenshot. The agent looks at the image, identifies the right element by visual cues (color, position, neighboring text), then clicks by ref — no coordinate guessing.

```bash
cu snapshot Mail --limit 50 --annotated --output /tmp/mail.png
# JSON includes: "annotated_screenshot": "/tmp/mail.png", "image_scale": 2.0
# (image_scale = pixel/point ratio, typically 2.0 on Retina)
```

The agent then opens `/tmp/mail.png`, sees boxes labeled `[3]`, `[7]`, `[12]`, picks one by visual identification, and runs `cu click 12 --app Mail`. This sidesteps the failure mode of "VLM picks coordinates that drift after a re-render".

### Cheap re-snapshots between actions

For multi-step flows, `cu snapshot --diff` returns only the elements that changed since the last snapshot of this app — usually a fraction of the full tree. The cache is per-pid at `/tmp/cu-snapshot-cache/<pid>.json`.

```bash
cu snapshot Mail --limit 100              # baseline (caches it)
cu click 12 --app Mail --no-snapshot       # do something
cu snapshot Mail --limit 100 --diff        # → +N ~M -K, usually << 100
```

The first `--diff` call (no cache) returns the full snapshot with `first_snapshot:true`. Identity is `(role, round(x), round(y))` — robust to ref re-numbering, sensitive to window movement (a moved window will show all elements as removed+added).

### Targeted query (preferred over `snapshot` + grep)

When you know what you're looking for, `cu find` is faster and cheaper than `cu snapshot --limit 200 | grep ...`:

```bash
# Find one element and act on it in two steps
cu find --app Mail --role textfield --title-contains "Subject" --first
# → {"match":{"ref":7,"role":"textfield",...},"count":1}
cu set-value 7 "Hello" --app Mail

# Or in one pipe
REF=$(cu find --app Safari --role button --title-equals "Reload" --first | jq -r .match.ref)
cu click "$REF" --app Safari

# Even simpler with --raw — bare integer, no jq needed
REF=$(cu find --app Safari --role button --title-equals "Reload" --first --raw)
cu click "$REF" --app Safari
```

`--raw` prints bare ref integers (one per line), exits 1 on no match — designed for `$(...)` substitution and shell pipelines.

Filters are AND-combined: `--role`, `--title-contains` (case-insensitive), `--title-equals`, `--value-contains`. Empty result is `ok:true` with `count:0` — not an error.

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
| `cu examples [topic]` | Built-in recipe library — try this when unsure how to chain commands |

### Scripting (for scriptable apps — check S flag in `cu apps`)
| Command | Description |
|---------|-------------|
| `cu tell <app> '<AppleScript>'` | Run AppleScript against app (read/write data) |

### System Preferences (bypass System Settings UI)
| Command | Description |
|---------|-------------|
| `cu defaults read <domain> [key]` | Read a macOS preference |
| `cu defaults write <domain> <key> <value>` | Write a macOS preference |

### Window management
| Command | Description |
|---------|-------------|
| `cu window list [--app Name]` | List all windows (title, position, size, state) |
| `cu window move <x> <y> --app Name` | Move window |
| `cu window resize <w> <h> --app Name` | Resize window |
| `cu window focus --app Name` | Bring app/window to front |
| `cu window minimize --app Name` | Minimize window |
| `cu window close --app Name` | Close window |

### Observe
| Command | Description |
|---------|-------------|
| `cu apps` | List running apps (`S` = scriptable, with class count) |
| `cu state <app>` | **First call when starting a task.** Snapshot + windows + screenshot + frontmost in one command — saves a round-trip vs. `snapshot` + `window list` + `screenshot` |
| `cu state <app> --no-screenshot` | Same, faster (skips the PNG capture) |
| `cu snapshot [app] --limit N` | AX tree with [ref] numbers |
| `cu snapshot [app] --diff` | Same, but only elements that changed since last snapshot of this app |
| `cu snapshot [app] --annotated --output path.png` | Same + writes a PNG with each ref's box+number drawn on it (for VLM agents) |
| `cu snapshot [app] --with-screenshot --output path.png` | Same + plain (un-annotated) window PNG, guaranteed same instant as the tree |
| `cu find --app X --role R --title-contains S` | Predicate query (preferred over snapshot+grep) |
| `cu nearest <x> <y> --app X` | Pixel → ref reverse lookup (for VLM agents that have visual coords) |
| `cu observe-region <x> <y> <w> <h> --app X` | All interactive refs whose bbox is in/touches a rect |
| `cu ocr [app]` | Vision OCR text recognition (for non-AX apps) |
| `cu screenshot [app] --path file.png` | Window capture (for visual analysis) |
| `cu screenshot --region "x,y WxH" --path file.png` | Capture only a screen rectangle (cheap VLM verification) |
| `cu wait --text "X" --app Name --timeout 10` | Poll until text/element appears |

### Act
| Command | Description |
|---------|-------------|
| `cu click <ref> --app Name` | Click + verify (default ON). Returns `verified: bool` and `verify_advice` when not verified — the safety net for sandboxed/Electron silent failures |
| `cu click <ref> --app Name --no-verify` | Skip verify (~50–150ms faster). Use only on known-reliable targets in bulk |
| `cu click <ref> --right` | Right-click |
| `cu click <ref> --double-click` | Double-click (open files, select words) |
| `cu click <ref> --shift` | Shift+click (extend selection) |
| `cu click <x> <y>` | Click screen coordinates |
| `cu key <combo> --app Name` | Keyboard shortcut (refused when frontmost is a terminal/IDE — pass `--allow-global` to override) |
| `cu type "text" --app Name` | Type text. Auto-routes via clipboard paste when text contains CJK or target is a chat app — see `paste_reason` in output |
| `cu type "text" --app Name --no-paste` | Force unicode-event delivery even on auto-paste-eligible inputs |
| `cu type "text" --app Name --paste` | Force paste (pbcopy + ⌘V) regardless of auto-detection |
| `cu set-value <ref\|--ax-path> "text" --app Name` | Write text into an AX field — no focus, no IME |
| `cu perform <ref\|--ax-path> <AXAction> --app Name` | Invoke a named AX action (`AXShowMenu`, `AXIncrement`, `AXScrollToVisible`, ...) |
| `cu scroll down 5 --x 400 --y 300` | Scroll |
| `cu hover <x> <y>` | Move mouse (tooltips) |
| `cu drag <x1> <y1> <x2> <y2>` | Drag |

### System
| Command | Description |
|---------|-------------|
| `cu setup` | Check permissions + version |
| `cu launch <name\|bundleId> [--no-wait] [--timeout 10]` | Launch app, wait for first window (auto-warms AX bridge) |
| `cu warm <app>` | Warm AX bridge for a manually-opened app (avoids 200–500ms first-snapshot cost) |
| `cu why <ref> --app Name` | Diagnose why a click/perform/set-value failed — returns enabled/in-bounds/actions/advice |

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

## Cookbook (10 high-frequency recipes)

Each recipe is the shortest correct sequence. Copy, swap names, run.

### 1. Launch an app and wait for it to be usable
```bash
cu launch TextEdit                 # waits up to 10s for AX-reported window
cu launch com.apple.Calculator     # bundle id form
cu launch Mail --no-wait           # spawn-and-go
```
Avoids the empty-AX-tree problem on cold starts. Returns `ready_in_ms` so the agent knows how long it took.

### 2. Read app data via AppleScript (preferred for scriptable apps)
```bash
cu apps                                                          # check S flag
cu sdef Calendar                                                 # discover schema
cu tell Calendar 'get summary of every event of first calendar'
```

### 3. Fill a form field without moving the cursor
```bash
REF=$(cu find --app Mail --role textfield --title-contains Subject --first --raw)
cu set-value "$REF" "Hello world" --app Mail   # method=ax-set-value, no focus theft
```
When `cu set-value` fails (Electron / non-AX field): `cu click "$REF" --app Mail; cu type "Hello world" --app Mail`.

### 3a. Multi-step flow: save axPath once, act on it across snapshots
```bash
# Snapshot once, capture stable selectors for the elements you'll act on:
SNAP=$(cu snapshot Mail --limit 100)
SUBJECT=$(echo "$SNAP" | jq -r '.elements[] | select(.role=="textfield" and (.title // "") | contains("Subject")) | .axPath')
SEND=$(echo "$SNAP" | jq -r '.elements[] | select(.role=="button" and (.title // "") == "Send") | .axPath')

# Now act — refs may have renumbered after each step, but axPaths still resolve:
cu set-value --ax-path "$SUBJECT" "Quarterly review" --app Mail
cu click --ax-path "$SEND" --app Mail
```
Use this whenever a step mutates the UI (opening a sheet, expanding a section). Refs would shift; axPaths don't.

### 4. Click a button when you only know its label
```bash
REF=$(cu find --app Safari --role button --title-equals Reload --first --raw)
cu click "$REF" --app Safari
```
Falls back to OCR when the button isn't in the AX tree:
```bash
cu click --text "Reload" --app Safari
```

### 5. Click by visual coordinates (VLM agent has a screenshot)
```bash
cu screenshot Safari --path /tmp/p.png             # agent looks at the image
REF=$(cu nearest 480 240 --app Safari | jq -r .ref) # pixel → nearest interactive ref
cu click "$REF" --app Safari
```

### 6. Scope work to a region (dialog / panel)
```bash
cu observe-region 480 200 400 300 --app Mail --mode center
# returns interactive refs whose centers fall inside the rect
```

### 7. Cheap visual verification (5–10× smaller than full window)
```bash
cu screenshot --region "480,200 400x300" --path /tmp/check.png
```

### 8. Wait for something to happen
```bash
cu wait --new-window     --app Mail --timeout 5     # sheet / new compose window
cu wait --modal          --app Finder --timeout 5   # save / replace dialog
cu wait --focused-changed --app Safari --timeout 5  # focus moved to next field
cu wait --text "Saved" --app TextEdit --timeout 10  # text appeared anywhere in the tree
```

### 9. See only what changed since last snapshot
```bash
cu snapshot Mail --diff
# {"diff": {"added": [...], "changed": [...], "removed": [...], "unchanged_count": N}}
```
Cuts token cost on long sessions where the UI mostly stays the same.

### 10. Read system preferences without opening Settings
```bash
cu defaults read com.apple.dock autohide
cu defaults write com.apple.dock autohide -bool true && killall Dock
```

### 11. Send a message in a chat / IM app (WeChat, Messages, Slack, Telegram)
Chat apps need behaviors different from normal AppKit apps. **The good news: cu does the right thing automatically.** You just need to know what the auto-routing means and what to do when it surfaces a problem.

1. They're often **partly sandboxed** — PID-targeted CGEvents may be silently ignored. `cu click` verifies by default and surfaces `verified: false` + `verify_advice` when this happens.
2. Their **rich-text editors drop the leading code units of unicode events** — non-ASCII (Chinese, Japanese, emoji) gets half-eaten. `cu type` auto-routes through clipboard paste when the text contains CJK or the target app is on the chat-app list (WeChat, Slack, Discord, Telegram, Lark, QQ, DingTalk, etc.). Look for `paste_reason` in the output.
3. **WeChat (and some Microsoft Office MAS builds) set `kCGWindowSharingState=0`** — capture-protected. `cu screenshot` and the screenshot embedded in `cu state` will refuse with `screenshot_error`; the AX tree (`elements`) still works. Use AX-based interaction; visual verification is impossible by design.

```bash
# 1. One call: tree + windows + capture-protected error (if any). The agent
#    sees screenshot_error in the result and knows to skip visual verification.
cu state WeChat

# 2. Find the message input field (rich textarea / textfield by role).
INPUT=$(cu find --app WeChat --role textarea --first --raw)

# 3. Click into the field, then type the message. Both auto-protect:
#    click verifies by default, type auto-pastes for CJK / chat apps.
cu click "$INPUT" --app WeChat                # verified by default — read `verified` + `verify_advice`
cu type "你好，这是来自 cu 的消息" --app WeChat # auto-paste, see `paste_reason`
cu key enter --app WeChat                     # send (some apps want cmd+enter — try enter first)
```

**If the click response has `verified: false`:** the target app silently dropped the event (sandboxed apps that ignore PID-targeted CGEvents). Read `verify_advice` for next-step instructions. Typical recovery: `cu window focus --app WeChat` (non-disruptive ax-raise) and retry, or pass `--allow-global` to fall back to the global-tap path (cursor moves but the click lands).

**If `cu state WeChat` returns `screenshot_error: "...capture-protected..."`:** that's expected. WeChat (and a few other privacy-conscious apps) opt out of screen capture at the OS level — both `CGWindowListCreateImage` and `ScreenCaptureKit` honor `kCGWindowSharingState=0`. cu cannot bypass it; this is the OS, not a bug. Drive the task with AX (snapshot/find/click) and accept that there's no visual verification.

## Key Rules

- **Always use `--app`** to target a specific app. Without it, keys go to the frontmost app which may have changed.
- **Refs are ephemeral** — they change after every UI mutation. Always re-snapshot.
- **Observe before acting** — don't guess refs from memory. Run `cu snapshot` first.
- **Verify after acting** — `cu snapshot` again to confirm the action worked.
- **JSON output** (when piped) auto-includes a fresh snapshot after click/key/type. Use `--no-snapshot` to disable.

## Focus Model (why `--app` matters)

`cu` is engineered to operate without disrupting the user. The mechanism is per-process event delivery: when `--app <Name>` is given, every CGEvent is posted to the target app's pid via `CGEventPostToPid` instead of the global HID tap.

**With `--app`:** the cursor stays where it is, the frontmost app stays frontmost, the clipboard is untouched, IME state is bypassed. `click`, `type`, `key`, `scroll`, `hover`, `drag`, `set-value`, `perform` all support this routing.

**Without `--app`:** events go through the global HID tap — the cursor warps, focus may shift, and the user notices.

The response includes a `method` field that documents which path was taken:

| method | meaning |
|--------|---------|
| `ax-action` | AX native action (no cursor move at all) — best |
| `ax-set-value` / `ax-perform` | direct AX attribute write / action call |
| `cgevent-pid`, `unicode-pid`, `key-pid`, `ocr-text-pid` | pid-targeted (non-disruptive) |
| `cgevent-global`, `unicode-global`, `key-global`, `ocr-text-global` | global HID tap (disruptive) |

If you see a `*-global` method in a response, it means `--app` was missing. Add it.

**Known limitations of pid-targeted delivery:**
- `drag` and `hover` move the cursor by design — pid-targeting suppresses focus theft but the cursor still moves to the target coordinates.
- A small set of sandboxed apps (some Mac App Store builds) ignore PID-targeted events. Symptom: action returns `ok:true` but the UI doesn't update. Workaround: focus the app first, then use `cu type` / `cu key` without `--app`.

## Output Format

When piped (default for agents), output is JSON:
```json
{"ok":true,"app":"Finder","window":"Downloads","elements":[{"ref":1,"role":"button","title":"Back","axPath":"window[Downloads]/toolbar/button[Back]","x":10,"y":40,"width":30,"height":24}],"displays":[{"id":1,"main":true,"x":0,"y":0,"width":1512,"height":982}]}
```

Each element carries both `ref` (cheap, ephemeral, refreshes per snapshot) and `axPath` (stable selector that survives UI churn — pass to action commands via `--ax-path` for multi-step flows).

Errors include context:
```json
{"ok":false,"error":"element [99] not found in AX tree (scanned 50 elements)"}
```

Action responses also carry these post-action fields:
- `method` — routing (`ax-action`, `cgevent-pid`, etc.)
- `confidence` — `high` / `medium` / `low`; check before relying on a coord-based or global-tap action
- `advice` — only present when not best-case (e.g. "pass --app to keep cursor put")
- `settle_ms` — actual ms waited via single-shot AXObserver (capped at 500ms)
- `snapshot` — fresh AX tree (skip with `--no-snapshot`)
- `verified` (`cu click` default-on) — pre/post AX diff; `false` means the click ran but the tree didn't change → `verify_advice` tells you what to do
- `verify_diff` — `{added, changed, removed}` element counts when verify ran
- `paste_reason` (`cu type`) — set when text was auto-routed through clipboard (CJK content or chat-app target)

Reliability advisories — when present, **read them**. They mean the result is degraded or auto-corrected:
- `truncation_hint` (snapshot) — `--limit` was hit; element you want may be past this batch. Re-run with bigger limit.
- `confidence_hint` (ocr) — at least one match is below 0.5 confidence. Vision returns plausible-looking hallucinations in this range.
- `screenshot_error` (`cu state`, screenshot path) — capture was refused, almost always `kCGWindowSharingState=0`. AX tree (`elements`) still works.
- `verify_advice` (click) — the action ran but didn't move the tree. Has concrete next-step instructions.

## Tips

- For Chrome/web tasks, prefer `bp` (browser-pilot) over `cu` — DOM-level precision beats AX tree for web content
- For desktop app tasks, `cu` is the right tool — AX tree provides element refs that Chrome CDP can't access
- If `cu snapshot` returns sparse results, try `cu ocr` (Vision OCR) or `cu screenshot` (visual fallback)
- Use `cu wait --text "X"` after actions that trigger loading or transitions
- Clipboard: use `pbcopy`/`pbpaste` directly, no need for `cu` wrapper
