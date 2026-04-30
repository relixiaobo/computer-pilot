# computer-pilot plugin

Agent plugin for [computer-pilot](https://github.com/relixiaobo/computer-pilot) — teach AI agents to control the macOS desktop via CLI.

## Install

### Step 1: Install cu binary

```bash
# Option A: Download prebuilt (macOS Apple Silicon)
sudo curl -Lo /usr/local/bin/cu https://github.com/relixiaobo/computer-pilot/releases/latest/download/cu-arm64 && sudo chmod +x /usr/local/bin/cu

# Option B: Build from source
git clone https://github.com/relixiaobo/computer-pilot.git
cd computer-pilot && cargo build --release
sudo cp target/release/cu /usr/local/bin/
```

### Step 2: Grant permissions

```bash
cu setup
```

### Step 3: Install plugin in Claude Code

```
/plugin marketplace add relixiaobo/computer-pilot
/plugin install computer-pilot@computer-pilot-marketplace
```

### Updating the plugin

```
/plugin marketplace update computer-pilot-marketplace
/plugin update computer-pilot@computer-pilot-marketplace
```

The `cu` binary is separate — re-run the curl command above to upgrade it.

## What it does

Adds a skill that teaches Claude Code how to use `cu` to:
- **Observe**: snapshot UI elements (AX tree), OCR, screenshots
- **Act**: click, type, keyboard shortcuts, scroll, drag (cursor stays put with `--app`)
- **Script**: AppleScript directly via `cu tell` for scriptable apps
- **Automate**: launch apps, navigate menus, fill forms, manage windows, change system preferences

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

## Commands (27)

| Category | Commands |
|---|---|
| **Discover** | `setup`, `apps`, `menu`, `sdef`, `examples` |
| **Observe** | `state`, `snapshot`, `find`, `nearest`, `observe-region`, `screenshot`, `ocr`, `wait` |
| **Act** | `click`, `type`, `key`, `set-value`, `perform`, `scroll`, `hover`, `drag` |
| **Script & System** | `tell`, `defaults`, `window`, `launch`, `warm`, `why` |

Run `cu <command> --help` for full per-flag reference, or `cu examples` for copy-paste recipes.

## Links

- [GitHub](https://github.com/relixiaobo/computer-pilot)
- [Full README](https://github.com/relixiaobo/computer-pilot#readme)
