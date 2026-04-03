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
| Commands | **12** | 7 | ~50 |

## Install

```bash
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

## Commands (12)

### Observe

| Command | Description |
|---------|-------------|
| `cu snapshot [app]` | AX tree with [ref] numbers, position, size |
| `cu screenshot [app]` | Silent window capture via CGWindowListCreateImage |
| `cu ocr [app]` | On-device OCR via macOS Vision framework |
| `cu wait --text/--ref/--gone` | Poll until UI condition is met |
| `cu apps` | List running applications |

### Act

| Command | Description |
|---------|-------------|
| `cu click <ref\|x y>` | Click (14-step AX chain, CGEvent fallback) |
| `cu click <ref> --right/--double-click/--shift` | Right-click, double-click, modifier keys |
| `cu key <combo> [--app]` | Keyboard shortcut (e.g., cmd+c, enter) |
| `cu type <text> [--app]` | Type text (Unicode supported) |
| `cu scroll <dir> <n> --x X --y Y` | Scroll up/down/left/right |
| `cu hover <x> <y>` | Move mouse (trigger tooltips) |
| `cu drag <x1> <y1> <x2> <y2>` | Drag with smooth interpolation |

### System

| Command | Description |
|---------|-------------|
| `cu setup` | Check permissions, version, and guide authorization |

## Output Modes

- **Human-readable** (TTY or `--human`): concise text
- **JSON** (piped/non-TTY): structured JSON for AI agents

Action commands (`click`, `key`, `type`) auto-return a fresh snapshot in JSON mode. Opt out with `--no-snapshot`.

## Perception Tiers

| Tier | Command | When to use | Token cost |
|------|---------|-------------|------------|
| 1 | `cu snapshot` | Default — native macOS apps, Chrome | Lowest |
| 2 | `cu ocr` | Apps with poor AX (games, Qt, Java) | Low |
| 3 | `cu screenshot` | Agent uses its own vision to analyze | Highest |

## Architecture

```
src/
  main.rs        CLI routing (clap), output formatting
  ax.rs          AX tree: batch reads, 14-step click chain, 3s timeout
  mouse.rs       CGEvent: click, scroll, hover, drag, modifiers
  key.rs         CGEvent keyboard + keycode mapping
  screenshot.rs  CGWindowListCreateImage + ImageIO PNG
  ocr.rs         Vision framework OCR via objc2
  system.rs      App resolution, permissions, System Events bridge
  wait.rs        Condition polling
```

## Permissions

`cu` requires two macOS permissions (run `cu setup` to check):

1. **Accessibility** — AX tree reading and input synthesis
2. **Screen Recording** — screenshot and OCR

## License

MIT
