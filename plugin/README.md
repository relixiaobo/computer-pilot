# computer-pilot plugin

Agent plugin for [computer-pilot](https://github.com/relixiaobo/computer-pilot) — teach AI agents to control the macOS desktop via CLI.

## Install

### Step 1: Install cu binary

```bash
# Option A: follow the checksum-verified Apple Silicon install in the main README

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

Every Agent product uses this same skill plus its existing shell. The plugin
does not install a separate slash-command adapter or native tool catalog.

## Commands (31)

| Category | Commands |
|---|---|
| **Discover** | `setup`, `apps`, `menu`, `sdef`, `examples` |
| **Observe** | `state`, `snapshot`, `find`, `nearest`, `observe-region`, `screenshot`, `ocr`, `wait` |
| **Act** | `click`, `type`, `key`, `set-value`, `perform`, `scroll`, `hover`, `drag` |
| **Script & System** | `tell`, `defaults`, `window`, `launch`, `warm`, `why` |
| **Recover** | `status`, `commands`, `command`, `cancel` |

Run `cu <command> --help` for full per-flag reference, or `cu examples` for copy-paste recipes.

## Links

- [GitHub](https://github.com/relixiaobo/computer-pilot)
- [Full README](https://github.com/relixiaobo/computer-pilot#readme)
