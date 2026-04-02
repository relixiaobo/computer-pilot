# Computer Pilot — Research & Design Document

## Market Landscape (as of April 2026)

### Commercial Products

| Product | Company | Approach | Platform | Key Data |
|---------|---------|----------|----------|----------|
| Computer Use | Anthropic | Screenshot + coordinates | macOS (Windows planned) | OSWorld 72.5%, near human level 72.4% |
| Operator (CUA) | OpenAI | Remote virtual browser + GPT-4o/5.4 | Web only | WebVoyager 87%, OSWorld 38.1% |
| Project Mariner | Google | Browser agent | Web only | N/A |
| Copilot Vision | Microsoft | Multi-model (Claude review + GPT generate) | Windows/Web | DRACO +13.8% |

### Open Source Projects

| Project | Stars | Language | Approach | Install | Key Feature |
|---------|-------|----------|----------|---------|-------------|
| UI-TARS-desktop (ByteDance) | High | TS | Multimodal vision + GUI Agent | CLI + Web UI | Most complete agent stack |
| Ghost OS | 1,276 | Swift | AX Tree + ShowUI-2B vision fallback | `brew install` | Self-learning recipes |
| open-computer-use (Coasty) | High | Python | Screenshot + CV | SDK | OSWorld 82%, production-grade |
| cua (trycua) | High | Python | Sandbox + SDK + benchmarks | SDK | Full-platform sandboxes |
| usecomputer | 214 | Zig | Screenshot + mouse/keyboard | `npm install -g` | Cross-platform CLI, lightest |
| agent-desktop | 36 | Rust | AX Tree + numbered refs | `npm install -g` | Closest to browser-pilot pattern |
| Open Interpreter | Very high | Python | Code execution + system commands | `pip install` | More code execution than GUI |
| Windows-Use | High | Python | Windows Accessibility Tree | Python SDK | Windows-specific |

### macOS Low-Level Tools

| Tool | Stars | Purpose | Install |
|------|-------|---------|---------|
| cliclick | 1,932 | Mouse/keyboard simulation | `brew install cliclick` |
| axcli | 8 | AX Tree + CSS selectors | `cargo install` |
| picc | 97 | Screenshot + OCR (Vision framework) | `cargo install` |

---

## Deep Dive: Key Projects

### Anthropic Computer Use

- **Pure vision**: Screenshot in, coordinates out. No accessibility tree.
- **Tool versions**: `computer_use_20241022` → `20250124` (adds scroll, hold_key, wait) → `20251124` (adds zoom)
- **Screenshot scaling**: Downscales to max 1280x800. Recommends 1024x768 for best accuracy.
- **Action loop**: screenshot → think → action → 2s delay → screenshot → repeat
- **Token cost**: ~1,400 tokens per screenshot (1280x800). 10-step task = 30,000-80,000+ tokens total.
- **Latency**: 3-8 seconds per action step (2s forced delay + API inference + capture).
- **March 2026 macOS release**: Tiered approach — native connectors → Chrome control → raw screen (last resort). Uses macOS accessibility layer (acquired Vercept AI).
- **Limitations**: Coordinate drift over multi-step tasks, small UI elements missed at high res, dynamic/animated UI confusion.

### Ghost OS (ghostwright/ghost-os)

- **Architecture**: MCP server (not CLI), Swift, single process.
- **AX capture**: Uses AXorcist (Swift wrapper). `AXUIElementCreateApplication(pid)` → recursive DFS to depth 25.
- **3-tier element finding**: AX tree search → CDP fallback (Chrome/Electron) → ShowUI-2B vision model.
- **Vision sidecar**: Separate Python process running ShowUI-2B (~3GB). First call 10-15s (model load), subsequent 0.5-3s.
- **Self-learning recipes**: CGEvent tap records user actions, AI synthesizes parameterized JSON recipes. Max 10 min recording.
- **Permissions**: Accessibility + Screen Recording + Input Monitoring. Granted to the terminal app, not Ghost OS itself.
- **Per-element timeout**: 3s (Chrome/Electron can hang on AX calls). Global AX timeout: 5s.
- **Limitations**: macOS only, Apple Silicon preferred, 3GB model download, Chrome AX tree is poor (AXGroup for everything), no multi-monitor.

### agent-desktop (lahfir/agent-desktop)

- **Architecture**: Rust CLI, npm-installable. 5-crate workspace.
- **AX capture**: `accessibility-sys` Rust crate → `AXUIElementCopyMultipleAttributeValues` (batch, 3-5x faster).
- **Tree traversal**: Recursive DFS. Nameless AXGroup nodes don't consume depth budget. Max default depth 10, cap 50.
- **Element refs**: `@e1`, `@e2`... assigned DFS order. Only 16 interactive roles get refs. Persisted to `~/.agent-desktop/last_refmap.json`.
- **Ref resolution**: Match by `(role, name, bounds_hash)`. Relaxed fallback: name-only if bounds shifted.
- **15-step click chain** (the most sophisticated part):
  1. AXPress (verified for lists) → 2. AXConfirm → 3. AXOpen → 4. AXPick
  5. ShowAlternateUI → 6. Child AXPress/Confirm/Open → 7. Value relay
  8. AXSelected=true → 9. Parent row select → 10. Parent table select
  11. Custom actions → 12. Focus + confirm/press → 13. Keyboard spacebar
  14. Ancestor press/confirm → **15. CGEvent mouse click (last resort)**
- **50 commands**: 10 app/window, 6 observation, 14 interaction, 2 scroll, 3 keyboard, 6 mouse, 3 clipboard, 1 wait, 3 system, 1 batch, 3+ notification.
- **Performance**: Batch attribute reads, 2s messaging timeout, FxHashSet for cycle detection, CI requires snapshot < 2s. Binary < 15MB.

### usecomputer (remorses/usecomputer)

- **Architecture**: Zig N-API native module + TypeScript CLI. Single codebase compiles to both .node addon and standalone binary.
- **Why Zig**: Zero-overhead `@cImport` for C FFI, cross-compiles from single host, produces both N-API lib and CLI.
- **Screenshot**: macOS=CGWindowListCreateImage, Linux=X11+XShm, Windows=GDI BitBlt. Scaled to max 1568px long edge.
- **coordMap**: Bridges image-space coordinates back to desktop-space. Format: `captureX,captureY,captureW,captureH,imageW,imageH`.
- **No AX support**: Purely coordinate-based. Designed as execution layer for AI vision models.
- **Commands**: screenshot, click, type, press, scroll, drag, hover, mouse move/position/down/up, debug-point, display-list, window-list.

### axcli (andelf/axcli)

- **Architecture**: Rust CLI using modern `objc2` bindings (not older `accessibility-sys`).
- **Playwright-style locators**: `#id`, `.class`, `Role[attr="val"]`, `text="VALUE"`, `text=/regex/`, `>>` descendant chain, `:has-text()`, `:has()`, `:visible`, `:nth-child()`, `:not()`.
- **Lazy & chainable**: Locator stores filter chain, AX tree only traversed when action is called.
- **Screenshot**: Both CGWindowListCreateImage and ScreenCaptureKit (per-window capture without focus).
- **Built-in OCR**: Apple Vision framework via `objc2-vision`.
- **Actions**: click, dblclick, input, fill, hover, focus, scroll-to, scroll, press, activate, wait, get, snapshot, screenshot.

---

## macOS Accessibility APIs — Technical Summary

### AXUIElement API
- IPC mechanism: every call crosses process boundaries to target app. Default timeout 6s.
- `AXUIElementCreateApplication(pid)` → recursive traversal via `kAXChildrenAttribute`.
- Batch reads with `AXUIElementCopyMultipleAttributeValues` (3-5x faster).
- Key attributes: AXRole, AXTitle, AXValue, AXDescription, AXPosition, AXSize, AXEnabled, AXFocused, AXIdentifier, AXDOMIdentifier, AXDOMClassList.
- Actions: AXPress, AXConfirm, AXOpen, AXPick, AXCancel, AXRaise, AXShowMenu, AXIncrement, AXDecrement.
- Performance: Full Safari tree ~15s. With depth limits (10-25) and batch reads: 50-300ms for most apps.

### App AX Quality
- **Good**: Native macOS apps, AppKit/SwiftUI, Chrome/Chromium (AXWebArea + DOM attributes).
- **Needs workaround**: Electron (set `AXManualAccessibility=true` to enable).
- **Poor**: Java Swing, Qt (inconsistent), games/Metal/OpenGL (zero AX tree).

### CGEvent (Input Synthesis)
- `CGEventCreateMouseEvent` for click/move/drag at specific coordinates.
- `CGEvent(keyboardEventSource:virtualKey:keyDown:)` for key events.
- `keyboardSetUnicodeString` for typing text.
- Requires Accessibility permission.
- Coordinate system: screen points, origin top-left of primary display.

### ScreenCaptureKit (Screenshots)
- `SCScreenshotManager.captureImage` — hardware-accelerated, 5-15ms.
- Per-window capture without bringing app to front.
- Requires Screen Recording permission.

### Vision Framework (OCR)
- `VNRecognizeTextRequest` — on-device, no network.
- `.accurate` mode: 200-500ms, excellent for screen content.
- `.fast` mode: 30-80ms, good for UI labels.
- Languages: English, Chinese, Japanese, Korean, French, German, Spanish, Portuguese, Italian, Russian, and many more.

### Permissions Required
| Permission | Required For | Check API |
|------------|-------------|-----------|
| Accessibility | AX tree + CGEvent input | `AXIsProcessTrusted()` |
| Screen Recording | ScreenCaptureKit screenshots | `CGPreflightScreenCaptureAccess()` |
| Input Monitoring | CGEvent tap (recording/learning) | Attempt `CGEvent.tapCreate()`, nil = denied |

Note: Permissions are granted to the **terminal app** (iTerm, Terminal.app), not the CLI binary itself.

---

## Design Decision

### Core Principle: Layered Perception, Graceful Degradation

```
Tier 1: AX Tree (text output, lowest tokens, highest precision)
  ↓ when AX info is insufficient
Tier 2: Screenshot + OCR (still text output, moderate tokens)
  ↓ when OCR can't identify elements
Tier 3: Screenshot image (visual, highest tokens — let agent's vision handle it)
```

Agent defaults to Tier 1 (cheapest). Only escalates when the app's AX support is poor.

### Command Design

```bash
# Perception
cu apps                         # list running apps
cu snapshot "Finder"            # AX tree snapshot with [ref] numbers
cu screenshot                   # screenshot (for non-AX apps)
cu ocr                          # screenshot + OCR (text output, cheaper than image)

# Actions (ref-first, coordinate fallback)
cu click 3                      # click [3] (AX-first 15-step chain)
cu click 500,300                # coordinate click (direct mouse)
cu type 5 "hello"               # type into [5]
cu type "hello"                 # type at current focus
cu key cmd+c                    # keyboard shortcut
cu scroll down 3                # scroll
cu drag 3 to 7                  # drag from [3] to [7]

# Window management
cu focus "Finder"               # bring app to front
cu windows                      # list windows

# Wait & verify
cu wait --text "Done"           # wait for text on screen
cu wait --gone 3                # wait for [3] to disappear

# System
cu permissions                  # check permission status
cu setup                        # guide user through authorization
```

### Architecture

```
cu (TypeScript CLI, npm distribution)
 │
 │  child_process.spawn (JSON over stdin/stdout)
 ▼
desktop-helper (Swift native binary, pre-built arm64 + x86_64)
 ├── AXUIElement API    → read AX tree, perform AX actions
 ├── CGEvent            → synthesize mouse/keyboard events
 ├── ScreenCaptureKit   → screenshots
 └── Vision Framework   → OCR
```

### Why This Architecture

**Swift for the native layer:**
- AXUIElement, CGEvent, ScreenCaptureKit, Vision are all macOS native APIs.
- Swift calls them directly, no FFI bindings needed.
- Rust (agent-desktop, axcli) works but needs `objc2` or `accessibility-sys` bridge layer.

**TypeScript for the CLI:**
- npm distribution (`npm install -g computer-pilot`) — same as browser-pilot.
- Command routing, JSON formatting, argument parsing.
- Swift binary pre-built and bundled in the npm package.

**JSON over stdin/stdout for the bridge:**
- Same pattern as AXorcist (by Ghost OS author).
- No native module compilation for users.
- Easy to test Swift and TypeScript sides independently.

### Differentiation

| | agent-desktop | Ghost OS | **computer-pilot** |
|---|---|---|---|
| Language | Rust | Swift | Swift + TypeScript |
| Distribution | npm | brew | **npm** |
| Perception | AX only | AX + 3GB vision model | **AX + OCR (zero extra download)** |
| Actions | 15-step AX chain | AX + synthetic click | **15-step AX chain (learned from agent-desktop)** |
| Visual fallback | None | ShowUI-2B (10-15s cold start) | **macOS Vision OCR (200ms, built-in)** |
| Plugin distribution | None | MCP only | **Claude Code Plugin + Codex Skills** |
| Pairs with browser-pilot | No | No | **Yes — unified experience, unified plugin** |

### Key Advantages

1. **Zero extra dependencies** — Unlike Ghost OS's 3GB model, OCR uses system-built-in Vision Framework.
2. **AX-first 15-step chain** — Learned from agent-desktop's most sophisticated design. Exhaust all AX approaches before mouse.
3. **Layered perception** — Default text output (save tokens), images only when needed.
4. **npm distribution + Plugin ecosystem** — Reuse browser-pilot's proven distribution pipeline.
5. **Complements browser-pilot** — Inside browser use `bp` (CDP, DOM-level precision), outside browser use `cu` (AX Tree, control-level precision).

### MVP Scope (6 commands)

1. `cu apps` — list running apps
2. `cu snapshot "AppName"` — AX tree with [ref] numbers
3. `cu click <ref|x,y>` — click (AX-first chain)
4. `cu type <ref|-> "text"` — type into element or current focus
5. `cu key <combo>` — keyboard shortcut
6. `cu screenshot` — capture screen

These 6 commands cover the core observe-act loop. Everything else can be added incrementally.

---

## Lessons from browser-pilot

Building browser-pilot taught us several hard-won lessons that directly apply to computer-pilot.

### 1. CLI output must be LLM-friendly, not human-friendly

The instinct is to make CLI output detailed and verbose. **This is wrong for agent tools.** LLMs have limited context windows and pay per token. Every extra character costs money and dilutes attention.

- **Return only what the agent needs for the next decision.** A snapshot should list interactive elements, not the entire page DOM.
- **Structured JSON when piped, human-readable when TTY.** Detect `process.stdout.isTTY` and switch format automatically.
- **Short, flat output beats nested, detailed output.** `[3] button "Submit"` is better than a 10-line JSON object describing that button.
- **Include hints in errors, not in success.** Don't explain what went well. Only explain what went wrong and how to fix it.

### 2. Less is more — fewer commands, each doing one thing well

browser-pilot started with many granular commands and consolidated them. The right number of commands is the minimum needed for a complete observe-act loop:

- **observe**: `snapshot` (what's on screen)
- **act**: `click`, `type`, `key` (interact with elements)
- **escape hatch**: `eval` in browser, `screenshot` + coordinates in desktop

Resist the urge to add `scroll`, `hover`, `drag`, `double-click` as separate commands in MVP. The agent can use `key`, `eval`, or coordinate clicks to achieve these. Add convenience commands only when agents repeatedly struggle.

### 3. Auto-snapshot after every action

This was browser-pilot's best design decision. After every `click`, `type`, or `press`, the tool automatically returns the updated page state. The agent never has to explicitly call `snapshot` — it always knows what happened.

Apply the same pattern: `cu click 3` should return the updated AX tree snapshot, not just `{"ok": true}`.

### 4. Numbered refs must be simple and deterministic

- **Sequential integers** (`[1]`, `[2]`, `[3]`), not hashes or UUIDs.
- **DFS order** so the numbers roughly follow visual top-to-bottom, left-to-right layout.
- **Only interactive elements** get refs. Static text, decorative images, layout groups should not pollute the list.
- **Refs refresh after every action.** Don't try to maintain stable refs across actions — it adds complexity and confuses the agent when the page changes.

### 5. The agent will misuse your tool — design for that

- Agents will call `click 99` when only 10 elements exist. Return a clear error with a hint: `"Ref [99] not found. Run 'cu snapshot' to refresh."`
- Agents will forget to `connect` first. Detect this and return: `"Not connected. Run 'cu setup' first."`
- Agents will pass wrong argument types. Be lenient in parsing: accept both `cu click 3` and `cu click "3"`.
- **Never fail silently.** Always return `{"ok": false, "error": "...", "hint": "..."}`.

### 6. Daemon architecture for stateful connections

browser-pilot uses a persistent daemon that maintains the CDP WebSocket. The CLI process is ephemeral — it sends a command to the daemon and exits. This is critical because:

- The agent spawns a new process for each command. Without a daemon, each command would need to re-establish the connection.
- State (current page, refs, auth credentials, network rules) lives in the daemon, not the CLI.

For computer-pilot, the native Swift helper should similarly be a **long-running process** that the TypeScript CLI communicates with, not a process that starts and dies with each command. AX tree caching, element resolution, and permission state all benefit from persistence.

### 7. Token budget awareness

Playwright MCP uses ~114,000 tokens per session. browser-pilot uses ~4,000-27,000 (4-35x reduction) because it outputs text, not screenshots.

For computer-pilot, the same principle: **text first, images only when needed.** An AX tree snapshot of Finder might be 50 elements × 30 characters = 1,500 tokens. A screenshot of the same window = ~1,400 tokens but with far less actionable information for the agent.

### 8. Permission and setup UX matters more than you think

Chrome's `chrome://inspect/#remote-debugging` toggle was simple — one URL, one click. For macOS desktop automation, you need THREE permissions (Accessibility, Screen Recording, Input Monitoring). Each requires navigating System Settings separately.

`cu setup` must be a guided wizard that:
- Checks each permission
- Opens the exact System Settings pane
- Waits and re-checks
- Provides clear feedback at each step

If setup is painful, nobody will use the tool. Invest in this early.

---

## References

### Commercial
- [Anthropic Computer Use Tool Docs](https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool)
- [OpenAI CUA API Docs](https://developers.openai.com/api/docs/guides/tools-computer-use)
- [Anthropic vs OpenAI CUA Comparison](https://workos.com/blog/anthropics-computer-use-versus-openais-computer-using-agent-cua)
- [Chrome Remote Debugging Changes](https://developer.chrome.com/blog/remote-debugging-port)

### Open Source
- [Ghost OS](https://github.com/ghostwright/ghost-os) — macOS desktop agent, MCP server, self-learning recipes
- [agent-desktop](https://github.com/lahfir/agent-desktop) — Rust CLI, 15-step AX chain, numbered refs
- [usecomputer](https://github.com/remorses/usecomputer) — Zig cross-platform CLI, coordinate-based
- [axcli](https://github.com/andelf/axcli) — Rust, Playwright-style AX locators
- [AXorcist](https://github.com/steipete/AXorcist) — Swift AX wrapper (used by Ghost OS)
- [UI-TARS Desktop](https://github.com/bytedance/UI-TARS-desktop) — ByteDance multimodal agent
- [CUA Framework](https://github.com/trycua/cua) — Sandboxed computer use infrastructure
- [open-computer-use](https://github.com/coasty-ai/open-computer-use) — OSWorld 82%

### macOS APIs
- [AXUIElement.h](https://developer.apple.com/documentation/applicationservices/axuielement_h)
- [ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit/)
- [VNRecognizeTextRequest](https://developer.apple.com/documentation/vision/vnrecognizetextrequest)
- [CGEvent](https://developer.apple.com/documentation/coregraphics/cgevent)
