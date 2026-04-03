# Computer Pilot — Docs

## Quick Start

```bash
cargo build --release
./target/release/cu setup
./target/release/cu --human apps
./target/release/cu --human snapshot Finder --limit 10
./target/release/cu --human click 3 --app Finder
./target/release/cu --human key cmd+c --app "Google Chrome"
./target/release/cu --human type "hello" --app "Google Chrome"
./target/release/cu --human screenshot "Google Chrome"
```

## Commands (12)

| Command | Purpose |
|---------|---------|
| `cu setup` | Check permissions, version, guide setup |
| `cu apps` | List running applications |
| `cu snapshot [app]` | AX tree snapshot with [ref] numbers |
| `cu click <ref\|x y>` | Click element (14-step AX chain, CGEvent fallback) |
| `cu key <combo>` | Keyboard shortcut (e.g., cmd+c, enter) |
| `cu type <text>` | Type text into focused element |
| `cu scroll <dir> <n>` | Scroll at coordinates |
| `cu hover <x> <y>` | Move mouse |
| `cu drag <x1> <y1> <x2> <y2>` | Drag |
| `cu screenshot [app]` | Silent window screenshot |
| `cu ocr [app]` | Vision OCR |
| `cu wait --text/--ref/--gone` | Wait for UI condition |

## Documents

| Document | Purpose |
|----------|---------|
| [CLAUDE.md](../CLAUDE.md) | Design rules and conventions |
| [competitive-analysis.md](competitive-analysis.md) | Feature comparison with competitors |
| [archive/](archive/) | Research and historical docs |
