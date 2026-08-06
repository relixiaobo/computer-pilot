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
| Commands | **31** | 7 | ~50 |

## Why cu doesn't disrupt your workflow

Most desktop-automation tools take focus from the user the moment they act:
the cursor jumps across the screen, the frontmost app changes, the clipboard
is overwritten. `cu` is engineered so the agent can work in the background
while you keep typing in your terminal.

| | cu | Codex Computer Use | Anthropic CUA | kagete |
|---|---|---|---|---|
| Cursor stays put | **✓ with `--app`** | ✓ | ✗ (warps) | ✗ (warps) |
| Frontmost app preserved | **✓ with `--app`** | ✓ | ✗ | ✗ |
| Clipboard untouched | **✓ except explicit/automatic paste** | ✓ | n/a | ✗ (paste-based) |
| IME bypassed | **✓ Unicode CGEvent** | ✓ | ✗ | ✗ |
| Perception | AX tree + OCR + screenshot | screenshot | screenshot | AX tree |
| AX action chain | **15-step fallback** | proprietary | n/a | basic AXPress |
| Method audit field | **✓** in every response | ✗ | ✗ | ✗ |

The mechanism is per-process event delivery: when `--app <selector>` is given,
every CGEvent is posted via `CGEventPostToPid` to the resolved pid instead
of through the global HID tap. The cursor and focus are not touched.

Selectors accept a unique app name, unique bundle ID, or `pid:<PID>`. When
development and production instances share a name or bundle ID, `cu` returns
`ambiguous_target` instead of guessing. Copy the intended process's `selector`
from `cu apps` and reuse it across state, action, and wait commands.

`cu type` injects UTF-16 directly via `CGEventKeyboardSetUnicodeString` —
no copy/paste, no clipboard pollution, works with any IME (Chinese, Japanese,
emoji). `cu key` posts virtual-key events to the same pid.

Every action response includes a `method` field documenting the routing:

| method | meaning |
|---|---|
| `ax-action`, `ax-set-value`, `ax-perform` | direct AX call, no cursor move at all |
| `cgevent-pid`, `unicode-pid`, `key-pid`, `ocr-text-pid` | PID-targeted (non-disruptive) |
| `cgevent-global`, `unicode-global`, `key-global`, `ocr-text-global` | global HID tap (disruptive — `--app` was missing) |

A `*-global` method in the response is the audit signal that the agent
forgot `--app` and disrupted the user. Always pass `--app <selector>`.

PID-targeted `drag` and `hover` also preserve the real cursor; only their
`*-global` variants move it. A small set of sandboxed Mac App Store apps
ignores PID-targeted events. `cu click` surfaces this as `verified:false`;
observe again and try another targeted AX primitive. Do not drop `--app` as a
workaround.

## Install

`cu` installs into a fixed, user-owned path —
`${XDG_DATA_HOME:-~/.local/share}/computer-pilot/bin/cu`. Upgrades replace
the same path atomically, which is what lets macOS TCC permissions survive
official upgrades. No sudo, and never `/usr/local/bin`.

**Put that directory first on your `PATH`.** macOS ships an unrelated
`/usr/bin/cu` (the UUCP serial dialer), so a plain `cu` resolves to it unless
Computer Pilot comes earlier:

```bash
echo 'export PATH="${XDG_DATA_HOME:-$HOME/.local/share}/computer-pilot/bin:$PATH"' >> ~/.zshrc
```

Verify with `cu --version`: Computer Pilot prints `cu <semver>`, the UUCP
dialer prints `cu (Taylor UUCP) 1.07`.

### Option A: Skill installer (recommended)

The Computer Pilot skill self-installs the exact CLI release pinned in its
`compatibility.json` on first use, verifying the SHA-256 sidecar and (for
Developer ID releases) the codesign requirement. Install the plugin (below)
and the binary follows automatically. To run the installer yourself:

```bash
git clone --depth 1 https://github.com/relixiaobo/computer-pilot.git
cd computer-pilot/plugin/skills/computer-pilot
sh scripts/install-native.sh \
  --version "$(sed -n 's/^  "version": "\(.*\)",$/\1/p' compatibility.json | head -1)" \
  --repository relixiaobo/computer-pilot --allow-unsigned
export PATH="${XDG_DATA_HOME:-$HOME/.local/share}/computer-pilot/bin:$PATH"
cu setup
```

Drop `--allow-unsigned` and pass
`--requirement "<installation.signing.requirement>"` once releases are
Developer ID signed (the manifest's `installation.signing` object declares
the current policy).

### Option B: Download binary manually (Apple Silicon)

Only Apple Silicon is supported. Download the binary, verify its published
SHA-256 checksum, and place it at the managed fixed path:

```bash
test "$(uname -m)" = "arm64"
workdir="$(mktemp -d)"
curl -fL -o "$workdir/cu-arm64" https://github.com/relixiaobo/computer-pilot/releases/latest/download/cu-arm64
curl -fL -o "$workdir/cu-arm64.sha256" https://github.com/relixiaobo/computer-pilot/releases/latest/download/cu-arm64.sha256
(cd "$workdir" && shasum -a 256 -c cu-arm64.sha256)
install_root="${XDG_DATA_HOME:-$HOME/.local/share}/computer-pilot"
mkdir -p "$install_root/bin"
chmod 0755 "$workdir/cu-arm64"
mv "$workdir/cu-arm64" "$install_root/bin/.cu.new"
mv "$install_root/bin/.cu.new" "$install_root/bin/cu"
export PATH="$install_root/bin:$PATH"
cu setup
```

### Option C: Build from source

```bash
git clone https://github.com/relixiaobo/computer-pilot.git
cd computer-pilot
cargo build --release
install_root="${XDG_DATA_HOME:-$HOME/.local/share}/computer-pilot"
mkdir -p "$install_root/bin"
cp target/release/cu "$install_root/bin/.cu.new"
mv "$install_root/bin/.cu.new" "$install_root/bin/cu"
export PATH="$install_root/bin:$PATH"
cu setup
```

### Claude Code Plugin

In Claude Code, run:

```
/plugin marketplace add relixiaobo/computer-pilot
/plugin install computer-pilot@computer-pilot-marketplace
```

This teaches Claude Code how to use `cu` automatically — just ask it to interact with desktop apps.

#### Updating the plugin

When a new version is released, update with:

```
/plugin marketplace update computer-pilot-marketplace
/plugin update computer-pilot@computer-pilot-marketplace
```

The updated skill pins a new CLI release in its `compatibility.json`; the
skill preflight converges the binary automatically on next use. Manual
installs (Option B/C) repeat the same steps to upgrade — the fixed path makes
the swap atomic.
Every release includes a binary archive, skill archive, plugin archive,
checksums, and `release-index.json`. Inspect `signing.status` in the index:
`ad-hoc-unsigned` releases are verified by their published SHA-256 digests but
carry no stable code identity — an ad-hoc designated requirement is a bare
cdhash that changes with every build. `developer-id-notarized` adds Apple-
verifiable provenance when signing credentials are available.

This does not normally affect permissions. macOS attributes `cu`'s
Accessibility and Screen Recording checks to the *responsible process* — the
terminal or Agent host that launched it — not to the `cu` binary, so the grant
lives on that app and upgrading `cu` does not disturb it. Run `cu setup` and
read `tcc_subject` to see which identity your setup actually uses.

### Agent Products

Computer Pilot has one public Agent integration: install the Computer Pilot
skill, provide the Agent's existing shell tool, and make `cu` available on
`PATH`. This is the same path used by self-installed, globally installed, and
product-bundled copies:

```text
Agent -> Computer Pilot skill -> existing shell tool -> cu CLI
      -> private per-user Broker -> macOS frameworks
```

Do not map commands to native Agent tools and do not add MCP, JSON-RPC, a
Native SDK, or an Agent-specific adapter. A product may bundle a pinned `cu`
binary, but it must invoke the normal CLI and use the same skill. See
[Universal Agent Integration](docs/universal-agent-integration.md).

## Quick Start

```bash
# What's running?
export COMPUTER_PILOT_CLIENT_KEY="example-agent"
export COMPUTER_PILOT_OUTPUT_DIR="/absolute/task-output"
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
cu screenshot "Google Chrome"

# OCR (for apps without good AX support)
cu ocr "Google Chrome"
# [100,200 300x20] "Example Domain" (100%)
# [100,240 500x16] "This domain is for use in..." (100%)
```

## Automation Commands (31)

### Discover

| Command | Description |
|---------|-------------|
| `cu apps` | List running apps (`S` flag = scriptable) |
| `cu menu <app>` | Enumerate any app's menu bar (works for ALL apps) |
| `cu sdef <app>` | Show scripting dictionary for scriptable apps |
| `cu examples [topic]` | Built-in recipe library (12 high-frequency tasks, copy-paste ready) |

### Observe

| Command | Description |
|---------|-------------|
| `cu snapshot [app]` | AX tree with [ref] numbers, position, size, window frame |
| `cu snapshot [app] --diff` | Only elements that changed since last snapshot of this app |
| `cu snapshot [app] --annotated --output p.png` | Captures window + draws each ref's box+number on it (for VLM agents) |
| `cu state <app>` | Snapshot + windows + screenshot + frontmost in one call (saves a round-trip when starting a task) |
| `cu find --role/--title-contains/--value-contains` | Predicate query — skip the `snapshot + grep` round-trip |
| `cu nearest <x> <y>` | Pixel → ref reverse lookup (for VLM agents that have visual coords) |
| `cu observe-region <x> <y> <w> <h>` | List interactive refs whose bbox is in/touches a rect (intersect/center/inside) |
| `cu screenshot [app]` | Silent window capture (ScreenCaptureKit primary path — works across Mission Control Spaces; refuses with `screenshot_error` for `kCGWindowSharingState=0` apps like WeChat) |
| `cu screenshot --region "x,y WxH"` | Capture a screen rectangle (5–10× smaller, for cheap VLM verification) |
| `cu ocr [app]` | On-device OCR via macOS Vision framework |
| `cu wait --text/--ref/--gone` | Poll until UI condition is met |

### Act

| Command | Description |
|---------|-------------|
| `cu click <ref\|x y\|--text>` | Click by ref, coordinates, or OCR text. Pre/post AX diff verifies by default — `verified:false` + `verify_advice` when sandbox apps swallow the event. `--no-verify` to skip |
| `cu key <combo> [--app]` | Keyboard shortcut (e.g., `cmd+c`, `enter`) |
| `cu type <text> [--app]` | Type text. Auto-routes via clipboard paste for Chromium inputs and known chat apps; native inputs keep Unicode events, including CJK — see `paste_reason` |
| `cu set-value <ref\|--ax-path> <text>` | Write text directly into an AX field — no focus, no IME, no clipboard |
| `cu perform <ref\|--ax-path> <action>` | Invoke a named AX action (`AXShowMenu`, `AXIncrement`, `AXScrollToVisible`, ...) |
| `cu scroll <dir> <n> --x --y --app <selector>` | Scroll in the target process without moving the real cursor |
| `cu hover <x> <y> --app <selector>` | Deliver mouse movement to the target process (trigger tooltips) without moving the real cursor |
| `cu drag <x1> <y1> <x2> <y2> --app <selector>` | Drag in the target process with smooth interpolation without moving the real cursor |

### Script & Control

| Command | Description |
|---------|-------------|
| `cu tell <app> <script>` | Run AppleScript against a scriptable app |
| `cu defaults read/write` | Read/write macOS preferences (no UI needed) |
| `cu window list/move/resize/focus/...` | Window management |
| `cu launch <name\|bundleId> [--no-wait]` | Launch app, wait for first window (auto-warms AX bridge) |
| `cu warm <app>` | Warm the AX bridge for a manually-opened app (avoids the 200–500ms first-snapshot cost) |
| `cu why <ref> --app <name>` | Diagnose why a click/perform/set-value failed — returns enabled/in-bounds/supported-actions/advice |
| `cu setup` | Check permissions and version |

### Recover

| Command | Description |
|---------|-------------|
| `cu status` | Broker health and command counts for this client |
| `cu commands` | List this client's command records |
| `cu command <id>` | Inspect one recoverable command |
| `cu cancel <id>` | Request cancellation; dispatched mutations may become uncertain |

Click supports: `--right`, `--double-click`, `--shift`, `--cmd`, `--alt`, `--text`, `--index`.

## How It Works

Three-tier control model — agent picks the cheapest layer for each task:

```
  Tier 1: AppleScript (scriptable apps)
  ┌──────────────────────────────────────────────┐
  │ cu tell <app> <script>   direct data access │
  │ cu sdef <app>            scripting dictionary│
  │ cu defaults read/write   system preferences  │
  └──────────────────────────────────────────────┘
                        ↓ fallback
  Tier 2: AX tree + CGEvent (any app)
  ┌──────────────────────────────────────────────┐
  │ cu snapshot   AX elements + window frame     │
  │ cu menu       menu bar via System Events     │
  │ cu window     list/move/resize/focus         │
  │ cu click      AX action → CGEvent fallback   │
  │ cu key/type   System Events / clipboard      │
  └──────────────────────────────────────────────┘
                        ↓ fallback
  Tier 3: OCR + screenshot (universal)
  ┌──────────────────────────────────────────────┐
  │ cu ocr           Vision OCR text + coords   │
  │ cu screenshot    PNG capture                 │
  │ cu click --text  click by OCR-found text     │
  └──────────────────────────────────────────────┘
```

**Perception tiers** (cheapest first):
1. `cu tell` — direct data, no UI traversal (scriptable apps only)
2. `cu snapshot` — structured AX tree text (~50 tokens/element)
3. `cu menu` — menu bar enumeration (when AX is sparse)
4. `cu ocr` — Vision OCR text + coordinates (for non-AX apps)
5. `cu screenshot` — image file (agent uses own vision)

## Output

**Human** (TTY or `--human`):
```
[app] Finder — "Downloads"
[1] button "Back" (10,40 30x24)
[2] statictext "Favorites" (10,100 80x16)
```

**JSON** (piped by default, or explicit with `--json`):
```json
{"schema_version":"1.0","ok":true,"app":"Finder","elements":[{"ref":1,"role":"button","title":"Back","x":10,"y":40,"width":30,"height":24}]}
```

Action commands auto-include a fresh snapshot in JSON mode. Use `--no-snapshot` to disable.

## Architecture

Single Rust binary. Zero runtime dependencies.

```
src/main.rs        CLI (clap) + output formatting
src/broker.rs      Private per-user coordination, Observations, recovery, locks
src/ax.rs          AX tree: batch reads, 15-step click chain, 3s timeout
src/mouse.rs       CGEvent: click, scroll, hover, drag, modifiers (PID-targeted)
src/key.rs         CGEvent keyboard, Unicode + keycode mapping (PID-targeted)
src/screenshot.rs  Window capture (ScreenCaptureKit primary, CGWindowList fallback)
src/sck.rs         ScreenCaptureKit sync wrapper (cross-Space, macOS 13+)
src/ocr.rs         Vision OCR via objc2
src/system.rs      App resolution, permissions, System Events, AppleScript tell
src/sdef.rs        Scripting dictionary extraction
src/wait.rs        Condition polling (--text/--ref/--gone/--new-window/--modal/--focused-changed)
src/diff.rs        Snapshot diff cache (cu snapshot --diff)
src/observer.rs    Single-shot AXObserver post-action settle wait
src/display.rs     CGGetActiveDisplayList + CGDisplayBounds
src/error.rs       Structured CuError type for actionable hints
src/file_result.rs Atomic 0600 file outputs, metadata, no-overwrite checks
```

## Permissions

Run `cu setup` to check and grant:

1. **Accessibility** — required for snapshot, click, key, type
2. **Screen Recording** — required for screenshot, OCR

Automation permission is requested only by Apple Events operations and is
granted separately for each target app; it is not one global readiness flag.

## License

MIT
