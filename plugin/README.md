# computer-pilot plugin

Agent plugin for [computer-pilot](https://github.com/relixiaobo/computer-pilot) — teach AI agents to control the macOS desktop via CLI.

## Install

### Step 1: Install cu binary

```bash
# Option A: Build from source
git clone https://github.com/relixiaobo/computer-pilot.git
cd computer-pilot && cargo build --release
sudo cp target/release/cu /usr/local/bin/

# Option B: Download prebuilt (macOS Apple Silicon)
curl -L https://github.com/relixiaobo/computer-pilot/releases/latest/download/cu-arm64 -o /usr/local/bin/cu
chmod +x /usr/local/bin/cu
```

### Step 2: Grant permissions

```bash
cu setup
```

### Step 3: Install plugin in Claude Code

```
/plugin marketplace add relixiaobo/computer-pilot
/plugin install computer-pilot@relixiaobo-computer-pilot
```

## What it does

Adds a skill that teaches Claude Code how to use `cu` to:
- **Observe**: snapshot UI elements (AX tree), OCR, screenshots
- **Act**: click, type, keyboard shortcuts, scroll, drag
- **Automate**: open apps, navigate menus, handle dialogs, file operations

## Usage

Once installed, Claude Code automatically uses `cu` when you ask it to interact with desktop apps:

```
"Open Calculator and compute 2+3"
"Take a screenshot of Chrome"
"Open System Settings and enable Dark Mode"
```

Or use the `/desktop` command:

```
/desktop open Finder and create a new folder called "test"
```

## Commands (12)

| Command | What it does |
|---------|-------------|
| `cu setup` | Check permissions |
| `cu apps` | List running apps |
| `cu snapshot` | AX tree with [ref] numbers |
| `cu click` | Click by ref or coordinates |
| `cu key` | Keyboard shortcut |
| `cu type` | Type text |
| `cu scroll` | Scroll |
| `cu hover` | Move mouse |
| `cu drag` | Drag |
| `cu screenshot` | Window capture |
| `cu ocr` | Vision OCR |
| `cu wait` | Wait for condition |

## Links

- [GitHub](https://github.com/relixiaobo/computer-pilot)
- [cu --help](https://github.com/relixiaobo/computer-pilot#commands-12)
