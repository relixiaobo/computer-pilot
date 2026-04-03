# cu — macOS Desktop Automation CLI

A single-binary CLI tool for AI agents to observe and control the macOS desktop. Built in Rust, zero runtime dependencies.

```
cu snapshot Finder --limit 10
cu click 3 --app Finder
cu key cmd+c --app "Google Chrome"
cu type "hello world" --app "Google Chrome"
cu screenshot "Google Chrome" --path /tmp/shot.png
cu ocr Finder
```

## Why cu?

| | cu | Anthropic Computer Use | agent-desktop |
|---|---|---|---|
| Binary | **1.3MB** | Python runtime | <15MB |
| Latency | **<10ms** | 3-8s per step | ~200ms |
| Dependencies | **zero** | Python | zero |
| Perception | AX tree + OCR + screenshot | screenshot only | AX tree only |
| Token cost | **text-first** (~50 tokens/element) | ~1400 tokens/screenshot | text-first |

## Install

```bash
# From source
git clone https://github.com/anthropics/computer-pilot.git
cd computer-pilot
cargo build --release
# Binary at ./target/release/cu
```

## Quick Start

```bash
# Check permissions
cu setup

# List running apps
cu apps

# Snapshot UI elements (AX tree with numbered refs)
cu snapshot "Google Chrome" --limit 20

# Click element by ref (AX action first, CGEvent fallback)
cu click 3 --app "Google Chrome"

# Keyboard shortcut
cu key cmd+t --app "Google Chrome"

# Type text
cu type "hello" --app "Google Chrome"

# Screenshot (no app activation needed)
cu screenshot "Google Chrome" --path /tmp/chrome.png

# OCR (macOS Vision framework)
cu ocr "Google Chrome"

# Wait for UI condition
cu wait --text "Submit" --app Safari --timeout 10
```

## Commands

### Perception

| Command | Description |
|---------|-------------|
| `cu snapshot [app]` | AX tree with [ref] numbers, position, size |
| `cu screenshot [app]` | Silent window capture via CGWindowListCreateImage |
| `cu ocr [app]` | On-device OCR via macOS Vision framework |
| `cu wait --text/--ref/--gone` | Poll until UI condition is met |
| `cu apps` | List running applications |

### Actions

| Command | Description |
|---------|-------------|
| `cu click <ref\|x y>` | Click (14-step AX chain, CGEvent fallback) |
| `cu click <ref> --right` | Right-click |
| `cu click <ref> --double-click` | Double-click |
| `cu click <ref> --shift` | Shift+click (also `--cmd`, `--alt`) |
| `cu scroll <dir> <n> --x X --y Y` | Scroll up/down/left/right |
| `cu hover <x> <y>` | Move mouse (trigger tooltips) |
| `cu drag <x1> <y1> <x2> <y2>` | Drag with smooth interpolation |

### Input

| Command | Description |
|---------|-------------|
| `cu key <combo>` | Keyboard shortcut (e.g., `cmd+c`, `enter`, `cmd+shift+s`) |
| `cu type <text>` | Type text (Unicode supported) |
| `cu key <combo> --app <name>` | Send to specific app via System Events |

### System

| Command | Description |
|---------|-------------|
| `cu setup` | Check Accessibility + Screen Recording permissions |
| `cu status` | Check helper status |
| `cu copy <text>` | Copy to clipboard |
| `cu paste` | Read clipboard |

## Output Modes

- **Human-readable** (TTY or `--human`): concise text output
- **JSON** (piped/non-TTY): structured JSON for AI agents

```bash
# Human
cu --human snapshot Finder --limit 3
# [app] Finder — "Downloads"
# [1] row "" (325,239 139x19)
# [2] cell "" (335,239 119x19)
# [3] statictext "Favorites" (340,238 121x21)

# JSON (auto when piped)
cu snapshot Finder --limit 3 | jq .
# {"ok":true,"app":"Finder","window":"Downloads","elements":[...],...}
```

## Auto-Snapshot

Action commands (`click`, `key`, `type`) automatically return a fresh snapshot in JSON mode, so the agent always knows the updated UI state:

```json
{
  "ok": true,
  "combo": "cmd+t",
  "snapshot": {
    "ok": true,
    "app": "Google Chrome",
    "elements": [...]
  }
}
```

Opt out with `--no-snapshot`.

## Perception Tiers

| Tier | Command | When to use | Token cost |
|------|---------|-------------|------------|
| 1 | `cu snapshot` | Default — native macOS apps, Chrome | Lowest |
| 2 | `cu ocr` | Apps with poor AX (games, Qt, Java) | Low |
| 3 | `cu screenshot` | Agent uses its own vision to look at the image | Highest |

## Architecture

```
src/
  main.rs        CLI routing (clap), output formatting
  ax.rs          AX tree walker, batch reads, 14-step click chain
  mouse.rs       CGEvent mouse: click, scroll, hover, drag, modifiers
  key.rs         CGEvent keyboard events + keycode mapping
  screenshot.rs  CGWindowListCreateImage + ImageIO PNG
  ocr.rs         Vision framework OCR via objc2
  system.rs      App resolution, permissions, System Events bridge
  wait.rs        Condition polling
```

- **Pure Rust** — single binary, no Node.js/Python/Swift
- **macOS FFI** — direct calls to AX, CGEvent, CoreGraphics, Vision, ImageIO
- **3 crate deps** — clap, serde, serde_json (+ objc2 for OCR)
- **3s per-element AX timeout** — prevents Chrome/Electron hangs
- **Batch AX reads** — `AXUIElementCopyMultipleAttributeValues` for 3-5x faster snapshots

## Permissions

`cu` requires two macOS permissions:

1. **Accessibility** — for AX tree reading and input synthesis
2. **Screen Recording** — for screenshot and OCR

Run `cu setup` to check and guide authorization.

## License

MIT
