# Computer Pilot — CLAUDE.md

## Project Overview

macOS desktop automation CLI (`cu`). Single Rust binary, zero runtime
dependencies. Every Agent uses the same public integration: the Computer Pilot
skill plus its existing shell plus the `cu` CLI. Internal Broker transport is
private and must not become an Agent API.
Three-tier control: **AppleScript** (scriptable apps) → **AX tree + CGEvent** (non-scriptable) → **OCR + screenshot** (fallback).

## Quick Reference

```
cargo build --release                         # Build
bash tests/commands/run_all.sh                # Run 700+ command-test assertions
./target/release/cu --human <command>         # Run in dev
bash scripts/release.sh <version>              # Prepare a draft release PR
bash scripts/release.sh <version> --dry-run    # Preview release-PR preparation
bash scripts/check-version-sync.sh             # Verify all version surfaces
```

## Release Flow

`scripts/release.sh` prepares a draft release PR:

1. **Pre-flight**: clean, current `main` matches `origin/main`, version/tag/branch are new
2. **Version bump**: update every CLI, skill, plugin, marketplace, and compatibility surface
3. **Build & test**: build plus L1/L2 checks unless explicitly skipped
4. **PR**: commit on `release/vX.Y.Z`, push that branch, and open a draft PR
5. **CI**: hosted static checks plus the TCC-enabled Apple Silicon command suite must pass
6. **Tag**: after merge, the protected Tag Release workflow tags the merged `main`
7. **Publish**: the tag workflow packages, checksums, and publishes assets; it
   signs/notarizes when Developer ID secrets exist, otherwise it emits a clearly
   marked fixed-identifier ad-hoc artifact

Manual rules:
- **Never push a release commit or tag directly to main.** Use a release PR and protected workflows.
- **README points to `/releases/latest/` URL** — auto-resolves to the newest release tag, so updating the release is enough.

The release script bumps **five** version surfaces in one commit:
1. `Cargo.toml` — drives `cu --version`
2. `plugin/.claude-plugin/plugin.json` — Claude Code plugin manifest
3. `plugin/package.json` — packaged skill/plugin version
4. `.claude-plugin/marketplace.json` — marketplace entry (what users see in `/plugin marketplace`)
5. `plugin/skills/computer-pilot/compatibility.json` — CLI, skill, platform, and Broker compatibility

All five must move together. `scripts/check-version-sync.sh` enforces this.

Users update the plugin with:
```
/plugin marketplace update computer-pilot-marketplace
/plugin update computer-pilot@computer-pilot-marketplace
```

## Architecture

Single Rust binary (`cu`). No TypeScript, Node.js, MCP server, public bridge,
Native SDK, or Agent-specific adapter. Agent products install the same skill
and invoke ordinary CLI commands through their existing shell. A private
per-user Broker may coordinate state and macOS permissions, but its transport
is never a public embedding surface.

```
src/main.rs        → CLI entry (clap), command routing, output formatting
src/broker.rs      → Private per-user Broker: commands, Observations, locks
src/ax.rs          → AX tree walker + AX actions (macOS Accessibility FFI)
src/mouse.rs       → Mouse operations (CGEvent FFI): click, scroll, hover, drag
src/key.rs         → Keyboard events (CGEvent FFI)
src/screenshot.rs  → Window capture (ScreenCaptureKit primary, CGWindowListCreateImage fallback)
src/sck.rs         → ScreenCaptureKit sync wrapper (cross-Space capable, macOS 13+)
src/ocr.rs         → OCR (macOS Vision framework via objc2)
src/system.rs      → NSWorkspace app identity, permissions, remaining System
                     Events bridges, tell, defaults, window mgmt, launch
src/sdef.rs        → Scripting dictionary parser (Rust native, quick-xml)
src/wait.rs        → UI condition polling (--text/--ref/--gone/--new-window/--modal/--focused-changed)
src/diff.rs        → Client-isolated private snapshot diff cache
src/file_result.rs → Atomic 0600 file outputs, metadata, no-overwrite safety
src/observer.rs    → Single-shot AXObserver post-action settle wait (D7)
src/display.rs     → CGGetActiveDisplayList + CGDisplayBounds (D1)
```

**31 public CLI commands**: 27 automation commands plus `status`, `commands`,
`command`, and `cancel` for recovery.

## Design Rules

These rules are derived from browser-pilot lessons and project experience. Follow them for all changes.

### 1. Output must be LLM-friendly

- **JSON when piped, human when TTY.** Detect via `process.stdout.isTTY`.
- **Keep it flat and short.** `[3] button "Submit" (10,40 30x24)` beats a 10-line JSON object.
- **Include hints only in errors, not in success.** Don't explain what went well.
- **Every element in snapshot must have a ref.** Only interactive roles get refs. Static layout elements are skipped.
- Always return `{"ok": false, "error": "...", "hint": "..."}` on failure. Never fail silently.

### 2. Auto-snapshot after every action

After `click`, `type`, `key`, the CLI automatically returns a fresh snapshot in JSON mode. This way the agent always knows the current UI state without an extra call.

- Add a **delay (~500ms) before post-action snapshot** so the UI has time to update.
- Opt out with `--no-snapshot` when the caller doesn't need it.

### 3. Ref design

- **Sequential integers** `[1]`, `[2]`, `[3]` in DFS order (roughly top-to-bottom, left-to-right).
- **Only interactive elements** get refs: button, textfield, textarea, statictext, row, cell, checkbox, radiobutton, popupbutton, combobox, link, menuitem, menubutton, tab, slider, image.
- **Refs are ephemeral.** They refresh with every snapshot. Don't try to keep stable refs across actions.

### 4. AX-first, CGEvent as fallback

When clicking, prefer **AX native actions** (AXPress, AXConfirm, AXOpen) over CGEvent coordinate clicks. AX actions are more reliable — they work even if the element is partially obscured. Only fall back to CGEvent mouse click when AX actions fail.

Implemented: `cu click <ref>` runs the 15-step AX action chain in `src/ax.rs` (AXPress → AXConfirm → AXOpen → ancestor walks → coord-derived ref → CGEvent fallback) and reports which step succeeded via the `method` field. CGEvent is only reached when every AX path is rejected.

### 5. Script-first for scriptable apps

When the target app is scriptable (`cu apps` shows `S` flag), prefer AppleScript
via `cu tell` over AX-based observe+click workflows. Scripting is:
- **Faster**: single step vs multi-step observe→click→verify
- **More reliable**: 85-95% vs 30-40% for complex tasks
- **Cheaper**: 50-200 tokens vs 2000+ for UI automation loops

Use `cu sdef <app>` to discover what an app supports via scripting.
Fall back to AX snapshot+click when:
- The app is not scriptable (Electron apps, Firefox, etc.)
- The task involves UI elements not exposed via the scripting dictionary
- The scripting approach fails

### 6. Focus model — `--app` and PID-targeted delivery

`cu`'s non-disruption guarantee comes from **per-process CGEvent delivery**:
when `--app <Name>` is given, every event is posted via `CGEventPostToPid`
to the resolved pid instead of through the global HID tap. The cursor stays
put, the frontmost app stays frontmost, and the user is not interrupted.

`--app` accepts a unique name, a unique bundle identifier, or `pid:<PID>`.
Names and bundle identifiers are never resolved by an active/first-instance
heuristic: if multiple GUI processes match, return `ambiguous_target` with
candidate selectors and do not observe or act. Reuse the chosen PID selector
through the state-act-verify loop, and refresh it after the process exits.

This applies to every action command: `click`, `type`, `key`, `scroll`,
`hover`, `drag`, `set-value`, `perform`. All of them resolve `--app` to a
pid up front and pass it down to `mouse::*` / `key::*`.

The `EventSource` RAII wrapper in `src/mouse.rs` and `src/key.rs` creates a
`kCGEventSourceStateCombinedSessionState` (=0) source when targeted, so PID
events do not collide with the user's real HID stream. Without `--app`, the
source is null (default global source) and events go through the global tap.

`cu type` uses **`CGEventKeyboardSetUnicodeString`** with `virtual_key=0` —
it injects UTF-16 code units directly per CGEvent, bypassing IME and the
clipboard. No pbcopy/pbpaste round-trip. `cu key` parses the combo and
posts virtual-key down/up events the same way.

Every action response carries a `method` field that documents the routing:
`ax-action`, `ax-set-value`, `ax-perform` (best — no cursor move at all),
`cgevent-pid` / `unicode-pid` / `key-pid` / `ocr-text-pid` (PID-targeted),
or `*-global` (global tap, disruptive). When debugging "did this disrupt
the user", grep for `*-global` in logs.

**Known limitation:** a small set of sandboxed apps (some Mac App Store
builds) ignore PID-targeted events. Symptom: `ok:true` returned but the UI
doesn't change. `cu click` catches this via the verify-by-default AX diff
(R2) — the response carries `verified:false` + `verify_advice` so the
agent can react. For a manual workaround: focus the app first, then retry
the same `cu click ... --app <Name>` (do NOT drop to global tap).

### 7. Screenshot rules

- **Rust-native** — `ScreenCaptureKit` primary path (cross-Space capable, macOS 13+); `CGWindowListCreateImage` fallback. No `screencapture` CLI.
- **No activation needed** — captures window content even when the app is behind other windows or on a different Space.
- **Window-scoped by default**, full screen with `--full`.
- Always return `offset_x`, `offset_y` in window mode: `screen = pixel + offset`.
- **Capture-protected windows refuse upfront** — when `kCGWindowSharingState=0` (WeChat etc.), return a structured error rather than a blank PNG. SCK and CGWindowList both honor this; it cannot be bypassed.

### 8. Agent operation etiquette

- Snapshot, screenshot, direct AX actions, and PID-targeted events preserve the
  user's frontmost app and real pointer. Always pass `--app`.
- `cu window focus`, commands without `--app`, `*-global` methods, and
  visualization mode (`--full -R`) are disruptive. Use them only when the
  workflow explicitly requires foreground state.
- Clipboard paste uses a desktop-level lock and restores the clipboard. Some
  Electron/CEF editors only accept paste while frontmost; a bounded exact-PID
  focus is visible to the user and must never happen as an unreported fallback.

### 9. Error handling

- All helper commands return `{"ok": true/false, ...}`. CLI must check `ok` and throw on failure.
- **snapshot `ok=false`** → throw, exit 1. Don't render an empty snapshot.
- **click `ok=false`** → throw, exit 1. Don't report a successful click.
- Include actionable hints: `"element [99] not found (snapshot was truncated at 50 — try --limit 100)"`.

### 10. Rust FFI conventions

- Rust 2024 edition: use `unsafe extern "C"` blocks.
- Use `#![allow(unsafe_op_in_unsafe_fn)]` at the top of FFI-heavy modules (ax.rs, mouse.rs, key.rs, screenshot.rs, ocr.rs).
- `cfstr()` returns `Option<CFStringRef>` — always handle null.
- All `AXUIElementCopyAttributeValue` results are +1 retained — caller must `CFRelease`.
- `CFArrayGetValueAtIndex` returns non-retained refs — keep the array alive while using them.
- Validate `is_finite()` on any user-provided `f64` before passing to FFI.

### 11. Security

- **AppleScript injection**: Escape `\` and `"` in user-provided text before embedding in AppleScript strings.
- **`cu tell` expressions**: The user/agent provides AppleScript expression, auto-wrapped in `tell application "X" ... end tell`. App name escaped via `applescript_escape()`. Timeout enforced (default 10s). Output uses `-ss` flag for unambiguous structured text.

## Agent Reliability Principles

Three principles govern every command's IO contract. Following them prevents
the *"agent acted on wrong information and didn't notice"* failure class —
the most lethal failure mode for an automation tool, because the agent has
no way to recover from input it doesn't know is wrong.

### Principle 1 — Single source of truth for identity

Anything that resolves "which window" or "which element" must go through AX.
`AXFocusedWindow` → `AXMainWindow` → `_AXUIElementGetWindow` for CGWindowID.
Do **not** pick a window from CGWindowList by heuristic (largest area, lowest
layer, first match). CGWindowList is a flat list with no semantic notion of
"the real window" — the same app typically has 3-5 layer-0 windows
(menu-bar proxy, AX helper, palette stubs) and choosing the wrong one
silently breaks every downstream command.

Implemented in `screenshot::find_window` (commit shipping R1). When adding a
new path that needs window identity, mirror this — never reach into
CGWindowList directly.

### Principle 2 — `ok=true` must mean it really happened

State-mutating actions (click/type/key) default to verifying their effect
via pre/post AX diff. Sandboxed and Electron apps return ok=true silently
when PID-targeted CGEvents are dropped — this is the #1 cause of "the agent
thinks it succeeded but the UI didn't change" debugging sessions.

Flag layout convention: `--no-verify` to opt out, **never** `--verify` to
opt in. New state-mutating commands follow the same pattern.

Implemented in `cu click` (R2 commit). When `verified=false`, attach a
`verify_advice` string with concrete remediation, not a generic "it didn't
work" — agents need next-step instructions, not just a flag.

### Principle 3 — Degraded output must be loud

When output is partial, low-confidence, omitted, or auto-routed, attach a
top-level **string** advisory the agent can read and react to. Boolean
flags (`truncated: true`) are easy for an agent to skip; explicit advisories
("snapshot stopped at 50 — re-run with `--limit 100`") are not.

Examples already wired:
- `truncation_hint` on `cu snapshot` (R3)
- `confidence_hint` on `cu ocr` when any recognition is below 0.5 (R6)
- `paste_reason` on `cu type` when auto-routed via clipboard (R7)
- `verify_advice` on `cu click` when verified=false (R2)
- `screenshot_error` on `cu state` and `cu screenshot` when capture refused (A from earlier batch)

Whenever a command returns degraded or auto-corrected output, follow the
same pattern: a string field whose name ends with `_hint` / `_reason` /
`_advice` / `_error`, populated only on the degraded path.

### Anti-patterns (concrete things NOT to reintroduce)

These are real bugs we've already paid for. The fix is in code; this
checklist exists so a future revert doesn't bring them back.

- **"Pick the largest / first / on-screen layer-0 window from CGWindowList"**
  — pick by AX, not heuristic. Rediscovered 2026-04: captured the menu-bar
  stub (3840×30) instead of the real off-Space window for TextEdit.
- **"Treat `AXTitle` as the user-visible label"** — Electron/CEF apps set
  AXTitle to internal IDs ("submit_btn_primary"). Walk
  AXTitle → AXDescription → AXHelp → AXIdentifier (R5).
- **"Return ok=true after a CGEvent click without verification"** —
  sandboxed apps silently drop PID-targeted events. Verify by AX diff (R2).
- **"Send unicode CGEvents to chat apps"** — WeChat/Slack/Discord/Telegram/
  Lark/QQ/DingTalk drop the first character. Auto-route through paste when
  text contains CJK or target is on the chat-app list (R7).
- **"Capture cross-Space window via CGWindowListCreateImage"** — returns
  blank PNG. Use ScreenCaptureKit (`sck.rs`) primary path (B), and honor
  `kCGWindowSharingState=0` upfront with a structured error (A).
- **"Truncate or low-confidence silently"** — attach an actionable hint
  string. Agents skim for unfamiliar string fields; they miss booleans.
- **"Use OnScreenOnly + first match heuristic"** — OnScreenOnly hides
  Mission Control windows, and "first match" depends on enumeration order.
  Both bite the moment conditions change.

## Testing

Three layers (defined in `tests/`):

- **L1 Command tests** (`tests/commands/run_all.sh`) — 700+ assertions covering every CLI command in isolation. The default run preserves the user's foreground app and pointer. Run `COMPUTER_PILOT_TEST_INTERACTIVE=1 bash tests/commands/run_all.sh` only on a dedicated desktop to include exact-PID focus, foreground Electron paste, and global HID compatibility tests. Specific suites: `bash tests/commands/run_all.sh snapshot key tell`.
- **L2 Agent E2E** (`tests/agent/run.py`) — real LLM agent + cross-check verification. Loads `plugin/skills/computer-pilot/SKILL.md` as the system prompt so the test mirrors production. Needs `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` in `.env`. Wired into `scripts/release.sh` — every release runs L2 unless `--skip-agent` or no API key.
- **L3 macOSWorld** (`tests/macosworld/`) — 133 locally-runnable tasks classified in `local_test_set.json`. Run via `tests/macosworld/run_selected.py`. Manual / quarterly cadence — too slow + heavy for per-release.

All tests use the release binary: `target/release/cu`. Build first with `cargo build --release`.

### Two rules for adding tests

These two rules close the gap that L1's 700+ structural assertions cannot
close on their own — *"the field is present"* says nothing about whether
the feature actually does what it was built for, and L1 can't see what an
LLM agent does with the skill.

**Rule 1 — Every new flag or output field must come with a behavior test**
that constructs the scenario the feature was built for and asserts the
user-visible state changed correctly. Structural assertions (field exists,
method equals X, error path returns non-zero) are necessary but never
sufficient. If a feature was added to disambiguate same-label-multi-pane,
the test must construct that situation and verify the click landed in the
right pane — not just verify an offscreen rect rejects.

Reference example: `tests/commands/test_region_disambiguation.sh` — opens
Finder home folder where folder names appear in both sidebar and main
pane, runs `cu click --text --region` against each, asserts the clicks
produced different coordinates (proving the filter actually filtered).

**Rule 2 — L2 agent E2E runs on every release**, not "when someone
remembers." `tests/agent/run.py` is wired into `scripts/release.sh`. It
catches what L1 cannot: agent training-prior regressions (inventing
flags, falling back to `osascript`, ignoring `verify_advice`). Skip with
`--skip-agent` only for emergency releases; mute by removing the API key
from `.env` only when explicitly debugging without LLM calls.

These rules don't replace L1 — they layer on top. L1 stays the
fast-feedback protocol guard.

## What NOT to do

- **Don't make this an MCP server.** This is a CLI tool, permanently. AI agents interact via CLI JSON output.
- Don't add commands for things the agent can achieve with existing commands (scroll = key down, hover = not needed in v1, double-click = two clicks).
- Don't add verbose success messages. `Clicked [3] button "OK"` is enough.
- Don't try to maintain stable refs across actions. Refs are cheap to regenerate.
- Don't use `screencapture` CLI. Use Rust-native ScreenCaptureKit (`sck.rs`, primary path) with `CGWindowListCreateImage` as fallback (`screenshot.rs`).
