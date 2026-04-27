# cu — macOS Desktop Automation CLI

A single-binary CLI tool for AI agents to observe and control the macOS desktop. Built in Rust, zero runtime dependencies.

```bash
cu snapshot Finder --limit 5
# [app] Finder — "Downloads"
# [1] button "Back" (10,40 30x24)
# [2] textfield "Search" (100,40 200x24)
# [3] statictext "Favorites" (10,100 80x16)
# [4] row "Documents" (10,120 300x20)
# [5] row "Desktop" (10,140 300x20)

cu click 4 --app Finder
# Clicked [4] via ax-action at (160, 130)
```

## Why cu?

| | cu | Anthropic Computer Use | agent-desktop |
|---|---|---|---|
| Size | **1.3MB** | Python runtime | <15MB |
| Latency | **<10ms** | 3-8s/step | ~200ms |
| Dependencies | **zero** | Python | zero |
| Perception | AX tree + OCR + screenshot | screenshot only | AX tree only |
| Token cost | **~50 tokens/element** | ~1400 tokens/screenshot | ~50 tokens/element |
| Commands | **24** | 7 | ~50 |

## Why cu doesn't disrupt your workflow

Most desktop-automation tools take focus from the user the moment they act:
the cursor jumps across the screen, the frontmost app changes, the clipboard
is overwritten. `cu` is engineered so the agent can work in the background
while you keep typing in your terminal.

| | cu | Codex Computer Use | Anthropic CUA | kagete |
|---|---|---|---|---|
| Cursor stays put | **✓ with `--app`** | ✓ | ✗ (warps) | ✗ (warps) |
| Frontmost app preserved | **✓ with `--app`** | ✓ | ✗ | ✗ |
| Clipboard untouched | **✓ always** | ✓ | n/a | ✗ (paste-based) |
| IME bypassed | **✓ Unicode CGEvent** | ✓ | ✗ | ✗ |
| Perception | AX tree + OCR + screenshot | screenshot | screenshot | AX tree |
| AX action chain | **15-step fallback** | proprietary | n/a | basic AXPress |
| Method audit field | **✓** in every response | ✗ | ✗ | ✗ |

The mechanism is per-process event delivery: when `--app <Name>` is given,
every CGEvent is posted via `CGEventPostToPid` to the resolved pid instead
of through the global HID tap. The cursor and focus are not touched.

`cu type` injects UTF-16 directly via `CGEventKeyboardSetUnicodeString` —
no copy/paste, no clipboard pollution, works with any IME (Chinese, Japanese,
emoji). `cu key` posts virtual-key events to the same pid.

Every action response includes a `method` field documenting the routing:

| method | meaning |
|---|---|
| `ax-action`, `ax-set-value`, `ax-perform` | direct AX call, no cursor move at all |
| `cgevent-pid`, `unicode-pid`, `key-pid`, `ocr-text-pid` | PID-targeted (non-disruptive) |
| `cgevent-global`, `unicode-global`, `key-global`, `ocr-text-global` | global HID tap (disruptive — `--app` was missing) |

A `*-global` method in the response is the audit signal that the agent
forgot `--app` and disrupted the user. Always pass `--app <Name>`.

**Known limitation:** `drag` and `hover` move the cursor by design. A small
set of sandboxed Mac App Store apps ignores PID-targeted events (symptom:
`ok:true` returned but the UI doesn't change) — focus the app first and
re-send without `--app` as the workaround.

## Install

### Option A: Download binary (Apple Silicon)

```bash
sudo curl -Lo /usr/local/bin/cu https://github.com/relixiaobo/computer-pilot/releases/latest/download/cu-arm64 && sudo chmod +x /usr/local/bin/cu && cu setup
```

### Option B: Build from source

```bash
git clone https://github.com/relixiaobo/computer-pilot.git
cd computer-pilot
cargo build --release
sudo cp target/release/cu /usr/local/bin/
cu setup
```

### Claude Code Plugin

In Claude Code, run:

```
/plugin marketplace add relixiaobo/computer-pilot
/plugin install computer-pilot-plugin@computer-pilot-marketplace
```

This teaches Claude Code how to use `cu` automatically — just ask it to interact with desktop apps.

#### Updating the plugin

When a new version is released, update with:

```
/plugin marketplace update computer-pilot-marketplace
/plugin update computer-pilot-plugin@computer-pilot-marketplace
```

The `cu` binary is separate — re-run the install curl command to upgrade it:

```bash
sudo curl -Lo /usr/local/bin/cu https://github.com/relixiaobo/computer-pilot/releases/latest/download/cu-arm64 && sudo chmod +x /usr/local/bin/cu
```

## Quick Start

```bash
# What's running?
cu apps
#  *S Finder (pid 572)
#     Google Chrome (pid 1551)

# See UI elements
cu snapshot "Google Chrome" --limit 10
# [1] button "Back" (5,84 34x34)
# [2] textfield "google.com" (157,89 939x24)
# [3] popupbutton "" (1149,84 34x34)
# ...

# Click element [2] (address bar)
cu click 2 --app "Google Chrome"

# Type a URL
cu type "https://example.com" --app "Google Chrome"

# Press Enter
cu key enter --app "Google Chrome"

# Wait for page load
cu wait --text "Example Domain" --app "Google Chrome" --timeout 10

# Screenshot (no activation needed — captures behind other windows)
cu screenshot "Google Chrome" --path /tmp/page.png

# OCR (for apps without good AX support)
cu ocr "Google Chrome"
# [100,200 300x20] "Example Domain" (100%)
# [100,240 500x16] "This domain is for use in..." (100%)
```

## Commands (26)

### Discover

| Command | Description |
|---------|-------------|
| `cu apps` | List running apps (`S` flag = scriptable) |
| `cu menu <app>` | Enumerate any app's menu bar (works for ALL apps) |
| `cu sdef <app>` | Show scripting dictionary for scriptable apps |
| `cu examples [topic]` | Built-in recipe library (12 high-frequency tasks, copy-paste ready) |

### Observe

| Command | Description |
|---------|-------------|
| `cu snapshot [app]` | AX tree with [ref] numbers, position, size, window frame |
| `cu snapshot [app] --diff` | Only elements that changed since last snapshot of this app |
| `cu snapshot [app] --annotated --output p.png` | Captures window + draws each ref's box+number on it (for VLM agents) |
| `cu find --role/--title-contains/--value-contains` | Predicate query — skip the `snapshot + grep` round-trip |
| `cu nearest <x> <y>` | Pixel → ref reverse lookup (for VLM agents that have visual coords) |
| `cu observe-region <x> <y> <w> <h>` | List interactive refs whose bbox is in/touches a rect (intersect/center/inside) |
| `cu screenshot [app]` | Silent window capture (no activation needed) |
| `cu screenshot --region "x,y WxH"` | Capture a screen rectangle (5–10× smaller, for cheap VLM verification) |
| `cu ocr [app]` | On-device OCR via macOS Vision framework |
| `cu wait --text/--ref/--gone` | Poll until UI condition is met |

### Act

| Command | Description |
|---------|-------------|
| `cu click <ref\|x y\|--text>` | Click by ref, coordinates, or OCR text |
| `cu key <combo> [--app]` | Keyboard shortcut (e.g., `cmd+c`, `enter`) |
| `cu type <text> [--app]` | Type text via Unicode CGEvent (IME-bypass, no clipboard) |
| `cu set-value <ref\|--ax-path> <text>` | Write text directly into an AX field — no focus, no IME, no clipboard |
| `cu perform <ref\|--ax-path> <action>` | Invoke a named AX action (`AXShowMenu`, `AXIncrement`, `AXScrollToVisible`, ...) |
| `cu scroll <dir> <n> --x --y` | Scroll up/down/left/right |
| `cu hover <x> <y>` | Move mouse (trigger tooltips) |
| `cu drag <x1> <y1> <x2> <y2>` | Drag with smooth interpolation |

### Script & Control

| Command | Description |
|---------|-------------|
| `cu tell <app> <script>` | Run AppleScript against a scriptable app |
| `cu defaults read/write` | Read/write macOS preferences (no UI needed) |
| `cu window list/move/resize/focus/...` | Window management |
| `cu launch <name\|bundleId> [--no-wait]` | Launch app, wait for first window (auto-warms AX bridge) |
| `cu warm <app>` | Warm the AX bridge for a manually-opened app (avoids the 200–500ms first-snapshot cost) |
| `cu why <ref> --app <name>` | Diagnose why a click/perform/set-value failed — returns enabled/in-bounds/supported-actions/advice |
| `cu setup` | Check permissions and version |

Click supports: `--right`, `--double-click`, `--shift`, `--cmd`, `--alt`, `--text`, `--index`.

## How It Works

Three-tier control model — agent picks the cheapest layer for each task:

```
  Tier 1: AppleScript (scriptable apps)
  ┌──────────────────────────────────────────────┐
  │ cu tell <app> <script>   direct data access │
  │ cu sdef <app>            scripting dictionary│
  │ cu defaults read/write   system preferences  │
  └──────────────────────────────────────────────┘
                        ↓ fallback
  Tier 2: AX tree + CGEvent (any app)
  ┌──────────────────────────────────────────────┐
  │ cu snapshot   AX elements + window frame     │
  │ cu menu       menu bar via System Events     │
  │ cu window     list/move/resize/focus         │
  │ cu click      AX action → CGEvent fallback   │
  │ cu key/type   System Events / clipboard      │
  └──────────────────────────────────────────────┘
                        ↓ fallback
  Tier 3: OCR + screenshot (universal)
  ┌──────────────────────────────────────────────┐
  │ cu ocr           Vision OCR text + coords   │
  │ cu screenshot    PNG capture                 │
  │ cu click --text  click by OCR-found text     │
  └──────────────────────────────────────────────┘
```

**Perception tiers** (cheapest first):
1. `cu tell` — direct data, no UI traversal (scriptable apps only)
2. `cu snapshot` — structured AX tree text (~50 tokens/element)
3. `cu menu` — menu bar enumeration (when AX is sparse)
4. `cu ocr` — Vision OCR text + coordinates (for non-AX apps)
5. `cu screenshot` — image file (agent uses own vision)

## Output

**Human** (TTY or `--human`):
```
[app] Finder — "Downloads"
[1] button "Back" (10,40 30x24)
[2] statictext "Favorites" (10,100 80x16)
```

**JSON** (piped — default for AI agents):
```json
{"ok":true,"app":"Finder","elements":[{"ref":1,"role":"button","title":"Back","x":10,"y":40,"width":30,"height":24}]}
```

Action commands auto-include a fresh snapshot in JSON mode. Use `--no-snapshot` to disable.

## Architecture

Single Rust binary. 9 source files, ~3000 lines.

```
src/main.rs        CLI (clap) + output formatting
src/ax.rs          AX tree: batch reads, 14-step click chain, 3s timeout
src/mouse.rs       CGEvent: click, scroll, hover, drag, modifiers
src/key.rs         CGEvent keyboard + keycode mapping
src/screenshot.rs  CGWindowListCreateImage + ImageIO
src/ocr.rs         Vision OCR via objc2
src/system.rs      App resolution, permissions, System Events, AppleScript tell
src/sdef.rs        Scripting dictionary extraction
src/wait.rs        Condition polling
```

## Permissions

Run `cu setup` to check and grant:

1. **Accessibility** — required for snapshot, click, key, type
2. **Screen Recording** — required for screenshot, OCR

## License

MIT
