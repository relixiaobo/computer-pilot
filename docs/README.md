# Computer Pilot — Docs

## Quick Start

```bash
cargo build --release
./target/release/cu --human apps
./target/release/cu --human snapshot Finder --limit 10
./target/release/cu --human click 3 --app Finder
./target/release/cu --human key cmd+c --app "Google Chrome"
./target/release/cu --human type "hello" --app "Google Chrome"
./target/release/cu --human screenshot "Google Chrome"
./target/release/cu --human setup
```

## Commands

| Command | Purpose |
|---------|---------|
| `cu setup` | Check permissions, guide setup |
| `cu status` | Check status |
| `cu apps` | List running applications |
| `cu snapshot [app]` | AX tree snapshot with [ref] numbers |
| `cu click <ref\|x y>` | Click element (AX action first, CGEvent fallback) |
| `cu key <combo>` | Keyboard shortcut (e.g., cmd+c, enter) |
| `cu type <text>` | Type text into focused element |
| `cu screenshot [app]` | Silent screenshot (no activation needed) |

## Documents

| Document | Purpose |
|----------|---------|
| [CLAUDE.md](../CLAUDE.md) | Design rules and conventions |
| [implementation-plan.md](implementation-plan.md) | Implementation progress |
| [archive/](archive/) | Research and historical docs |
