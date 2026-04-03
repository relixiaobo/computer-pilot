# computer-pilot plugin

Agent plugin for [computer-pilot](https://github.com/anthropics/computer-pilot) — teach AI coding agents to control the macOS desktop via CLI.

## What it does

Adds a skill that teaches Claude Code (and other AI agents) how to use the `cu` CLI tool to:
- Observe desktop UI via Accessibility tree, OCR, and screenshots
- Click buttons, type text, send keyboard shortcuts
- Navigate macOS apps (Spotlight, menus, dialogs)
- Automate desktop workflows

## Install

### 1. Install cu binary

```bash
git clone https://github.com/anthropics/computer-pilot.git
cd computer-pilot
cargo build --release
sudo cp target/release/cu /usr/local/bin/
cu setup   # grant permissions
```

### 2. Install plugin

```bash
# Via Claude Code
/install-plugin computer-pilot-plugin

# Or manually
cd plugin && npm pack
```

## Usage

Once installed, Claude Code automatically activates the skill when you ask it to interact with desktop apps:

- "Open Calculator and compute 2+3"
- "Take a screenshot of Chrome"
- "Find the Submit button in Safari and click it"
- "Open System Settings and change the wallpaper"

Or use the `/desktop` command directly:

```
/desktop open Finder and create a new folder called "test"
```

## Skill content

The plugin teaches the agent:
- **Core workflow**: observe → act → verify
- **12 commands**: snapshot, click, key, type, scroll, hover, drag, screenshot, ocr, wait, apps, setup
- **macOS patterns**: Spotlight app launch, menu navigation, dialog handling, clipboard
- **Perception tiers**: AX tree (cheapest) → OCR → screenshot (most expensive)
- **Best practices**: always use --app, refs are ephemeral, verify after every action
