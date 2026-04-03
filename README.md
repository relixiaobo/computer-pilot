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
| Commands | **12** | 7 | ~50 |

## Install

### Option A: Download binary (Apple Silicon)

```bash
curl -L https://github.com/relixiaobo/computer-pilot/releases/latest/download/cu-arm64 -o /usr/local/bin/cu
chmod +x /usr/local/bin/cu
cu setup   # grant Accessibility + Screen Recording
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

```
/install-plugin computer-pilot-plugin
```

This teaches Claude Code how to use `cu` automatically — just ask it to interact with desktop apps.

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

## Commands (12)

### Observe

| Command | Description |
|---------|-------------|
| `cu snapshot [app]` | AX tree with [ref] numbers, position, size |
| `cu screenshot [app]` | Silent window capture (no activation needed) |
| `cu ocr [app]` | On-device OCR via macOS Vision framework |
| `cu wait --text/--ref/--gone` | Poll until UI condition is met |
| `cu apps` | List running applications |

### Act

| Command | Description |
|---------|-------------|
| `cu click <ref\|x y>` | Click (14-step AX chain → CGEvent fallback) |
| `cu key <combo> [--app]` | Keyboard shortcut (e.g., `cmd+c`, `enter`) |
| `cu type <text> [--app]` | Type text (Unicode supported) |
| `cu scroll <dir> <n> --x --y` | Scroll up/down/left/right |
| `cu hover <x> <y>` | Move mouse (trigger tooltips) |
| `cu drag <x1> <y1> <x2> <y2>` | Drag with smooth interpolation |

### System

| Command | Description |
|---------|-------------|
| `cu setup` | Check permissions and version |

Click supports: `--right`, `--double-click`, `--shift`, `--cmd`, `--alt`.

## How It Works

```
                  Observe                          Act
              ┌─────────────┐              ┌──────────────┐
  cu snapshot │ AX Tree API │  cu click    │ AXPress      │
              │ batch reads │  cu key      │ → AXConfirm  │
              │ 3s timeout  │  cu type     │ → AXOpen     │
              └─────────────┘  cu scroll   │ → ...12 more │
  cu ocr      ┌─────────────┐  cu drag     │ → CGEvent    │
              │ Vision OCR  │  cu hover    │   (fallback) │
              └─────────────┘              └──────────────┘
  cu screenshot┌─────────────┐
              │ CGWindowList │  JSON output with auto-snapshot
              │ (no activate)│  after every action
              └─────────────┘
```

**Perception tiers** (cheapest first):
1. `cu snapshot` — structured AX tree text (~50 tokens/element)
2. `cu ocr` — Vision OCR text + coordinates (for non-AX apps)
3. `cu screenshot` — image file (agent uses own vision)

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

Single Rust binary. 8 source files, ~2500 lines.

```
src/main.rs        CLI (clap) + output formatting
src/ax.rs          AX tree: batch reads, 14-step click chain, 3s timeout
src/mouse.rs       CGEvent: click, scroll, hover, drag, modifiers
src/key.rs         CGEvent keyboard + keycode mapping
src/screenshot.rs  CGWindowListCreateImage + ImageIO
src/ocr.rs         Vision OCR via objc2
src/system.rs      App resolution, permissions, System Events
src/wait.rs        Condition polling
```

## Permissions

Run `cu setup` to check and grant:

1. **Accessibility** — required for snapshot, click, key, type
2. **Screen Recording** — required for screenshot, OCR

## License

MIT
