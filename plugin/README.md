# computer-pilot plugin

Agent plugin for [computer-pilot](https://github.com/relixiaobo/computer-pilot) — teach AI agents to control the macOS desktop via CLI.

## Install

### Install the plugin in Claude Code

```
/plugin marketplace add relixiaobo/computer-pilot
/plugin install computer-pilot@computer-pilot-marketplace
```

The skill installs the exact CLI release pinned in `compatibility.json` on first
use, verifies its checksum and declared signing identity, and uses a fixed
user-owned path: `${XDG_DATA_HOME:-~/.local/share}/computer-pilot/bin/cu`.
Keep that directory before `/usr/bin` on `PATH` so macOS's unrelated UUCP `cu`
does not shadow it. Run `cu setup` and follow its permission guidance.

For manual installation or a source build, use the
[main install guide](../README.md#install). Agent hosts should consume
`skill-stable`; the [release checkpoint](../docs/ROADMAP.md#release-status)
records the latest completed promotion.

### Updating the plugin

```
/plugin marketplace update computer-pilot-marketplace
/plugin update computer-pilot@computer-pilot-marketplace
```

The updated skill pins its matching CLI version. Its preflight upgrades the
managed binary atomically on next use; manual installations follow the main
install guide. No sudo or `/usr/local/bin` copy is needed.

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
