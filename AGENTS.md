# Computer Pilot — AGENTS.md

> Same working agreement as `CLAUDE.md`, kept under a second name so Codex picks
> it up automatically. Update both when the project's contract changes (or
> symlink one to the other).

## Project Overview

macOS desktop automation CLI (`cu`). Single Rust binary, zero runtime dependencies.
Three-tier control: **AppleScript** (scriptable apps) → **AX tree + CGEvent** (non-scriptable) → **OCR + screenshot** (fallback).

## Quick Reference

```
cargo build --release                         # Build
bash tests/commands/run_all.sh                # Run 470+ command tests
./target/release/cu --human <command>         # Run in dev
bash scripts/release.sh <version>             # Release: bump → tag → push → GitHub
bash scripts/release.sh <version> --dry-run   # Dry run first
```

## Release Flow

`scripts/release.sh` automates the full release pipeline:

1. **Pre-flight**: clean tree, on `main`, in sync with `origin`, tag/release don't exist, `gh` authenticated
2. **Version bump**: updates `Cargo.toml`
3. **Build & test**: `cargo build --release` + `bash tests/commands/run_all.sh` (must pass)
4. **Commit**: `Bump version to X.Y.Z`
5. **Push**: commit + `vX.Y.Z` tag to `origin/main`
6. **GitHub release**: upload `cu-arm64` binary, generate notes from commits since last tag

Manual rules:
- **Never push directly to main without a release if there are user-visible changes.** Bump the version and run `release.sh` so the published binary stays in sync with `README.md` install instructions.
- **README points to `/releases/latest/` URL** — auto-resolves to the newest release tag, so updating the release is enough.

The release script bumps **three** version numbers in one commit:
1. `Cargo.toml` — drives `cu --version`
2. `plugin/.claude-plugin/plugin.json` — Claude Code plugin manifest
3. `.claude-plugin/marketplace.json` — marketplace entry (what users see in `/plugin marketplace`)

All three must move together. Claude Code only detects a plugin update if `marketplace.json` version changes.

Users update the plugin with:
```
/plugin marketplace update computer-pilot-marketplace
/plugin update computer-pilot-plugin@computer-pilot-marketplace
```

## Architecture

Single Rust binary (`cu`). No TypeScript, no Node.js, no IPC.

```
src/main.rs        → CLI entry (clap), command routing, output formatting
src/ax.rs          → AX tree walker + AX actions (macOS Accessibility FFI)
src/mouse.rs       → Mouse operations (CGEvent FFI): click, scroll, hover, drag
src/key.rs         → Keyboard events (CGEvent FFI)
src/screenshot.rs  → Window capture (CGWindowListCreateImage + ImageIO)
src/ocr.rs         → OCR (macOS Vision framework via objc2)
src/system.rs      → App resolution, permissions, System Events bridges:
                     tell, menu, defaults, window mgmt, type/key, launch
src/sdef.rs        → Scripting dictionary parser (Rust native, quick-xml)
src/wait.rs        → UI condition polling (--text/--ref/--gone/--new-window/--modal/--focused-changed)
src/diff.rs        → Snapshot diff cache (cu snapshot --diff)
src/observer.rs    → Single-shot AXObserver post-action settle wait (D7)
src/display.rs     → CGGetActiveDisplayList + CGDisplayBounds (D1)
```

**24 commands** across discovery, observation, action, scripting, system control.

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

Current state: click only uses CGEvent. **TODO**: Implement the AX action chain before CGEvent fallback.

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
doesn't change. Workaround: focus the app first, then send keys without
`--app`. Detecting this automatically (D4 in `docs/ROADMAP.md`) is deferred
to Sprint 2 because it requires the diff-snapshot machinery (C1).

### 7. Screenshot rules

- **Rust-native** — uses `CGWindowListCreateImage` (no `screencapture` CLI).
- **No activation needed** — captures window content even when the app is behind other windows.
- **Window-scoped by default**, full screen with `--full`.
- Always return `offset_x`, `offset_y` in window mode: `screen = pixel + offset`.

### 8. Agent operation etiquette

- When the agent operates another app (click, key, type, screenshot), it takes focus away from the user's terminal. **Minimize disruption time.**
- Screenshot is observation-only — but still needs app activation for `-R` mode. Accept this limitation for v1.
- Future: consider `ScreenCaptureKit` for per-window capture without activation.

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

## Testing

Three layers (defined in `tests/`):

- **L1 Command tests** (`tests/commands/run_all.sh`) — 258 assertions covering every CLI command in isolation. Run: `bash tests/commands/run_all.sh` or `bash tests/commands/run_all.sh snapshot key tell` for specific suites.
- **L2 Agent E2E** (`tests/agent/run.py`) — real LLM agent + cross-check verification. Loads `plugin/skills/computer-pilot/SKILL.md` as the system prompt so the test mirrors production. Needs `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` in `.env`.
- **L3 macOSWorld** (`tests/macosworld/`) — 133 locally-runnable tasks classified in `local_test_set.json`. Run via `tests/macosworld/run_selected.py`.

All tests use the release binary: `target/release/cu`. Build first with `cargo build --release`.

## What NOT to do

- **Don't make this an MCP server.** This is a CLI tool, permanently. AI agents interact via CLI JSON output.
- Don't add commands for things the agent can achieve with existing commands (scroll = key down, hover = not needed in v1, double-click = two clicks).
- Don't add verbose success messages. `Clicked [3] button "OK"` is enough.
- Don't try to maintain stable refs across actions. Refs are cheap to regenerate.
- Don't use `screencapture` CLI. Use Rust-native `CGWindowListCreateImage` instead (screenshot.rs).
