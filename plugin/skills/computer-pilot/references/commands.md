# Computer Pilot CLI — Full Command Reference

Read this file when you need exhaustive flag-level detail for a `cu` command — e.g. all the modifiers `cu click` accepts, the full set of `cu wait` conditions, every field in a `cu state` response. The main `SKILL.md` covers the load-bearing 80%; this file covers the rest.

All commands return JSON when piped; pass `--human` to force readable output.

---

## Discovery

### `cu setup`
Check Accessibility / Screen Recording / Automation permissions and report cu version. Opens System Settings if a permission is missing. Run once after install.

### `cu apps`
List every running app: `name`, `pid`, `active` (frontmost), `scriptable` (`S` flag), `sdef_classes` (count of scripting classes — higher = richer dictionary). Use the `S` flag to decide whether to reach for `cu tell`.

### `cu menu <app>`
Enumerate every menu and menu item in the app's menu bar via System Events. Works for **any** app — scriptable or not. Returns menu name, item name, and enabled status. After discovering, click any item via:
```bash
cu tell "System Events" 'tell process "<App>" to click menu item "<Item>" of menu "<Menu>" of menu bar 1'
```

### `cu sdef <app>`
Show the AppleScript scripting dictionary — classes, properties, commands. Pure-Rust XML parsing, no external tools. Use this before writing `cu tell` to know what the app supports.

### `cu examples [topic]`
Built-in recipe library. `cu examples` lists topics; `cu examples launch-app` (etc.) prints the recipe. Useful when you forget how a particular workflow chains.

---

## Observation

### `cu state <app> [--limit N] [--no-screenshot] [--output P]`
**Canonical "start a task" call.** Returns snapshot + windows + screenshot path + frontmost in one call — replaces four separate round-trips (`cu apps` + `cu window list` + `cu snapshot` + `cu screenshot`).

Flags:
- `--limit N` — AX tree depth limit (default 50)
- `--no-screenshot` — skip the screenshot capture (faster, tree+windows only)
- `--output P` — output path for the window screenshot (default `/tmp/cu-state-<ts>.png`)

Output fields:
- `app`, `pid`, `frontmost`, `windows[]`, `displays[]`, `elements[]`
- `snapshot_size`, `tree_truncated`
- `screenshot` — PNG path (omitted with `--no-screenshot`)
- `image_scale` — pixel/point ratio (typically 2.0 on Retina)
- `screenshot_error` — string when capture refused (`kCGWindowSharingState=0`); AX tree still works, drive task without visual verification
- `truncation_hint` — when snapshot was clipped

### `cu snapshot [app] [--limit N] [--diff] [--annotated --output P] [--with-screenshot --output P]`
AX tree of interactive UI elements. Each element has `ref`, `role`, `title`, `value`, `axPath`, `x`, `y`, `width`, `height`.

Flags:
- `--limit N` — max elements (default 50). If hit, response carries `truncation_hint` — re-run with larger limit.
- `--diff` — return only elements that changed since the previous snapshot of this app. Cache lives at `/tmp/cu-snapshot-cache/<pid>.json`. First call returns full snapshot with `first_snapshot:true`.
- `--annotated --output path.png` — writes a PNG with each ref's bounding box + number drawn. Highest-leverage flow for VLM agents (the model sees the UI with refs already labeled).
- `--with-screenshot --output path.png` — captures a plain (un-annotated) PNG in the same instant as the tree. Use when you want both tree and image but don't want refs drawn on the image.

Only interactive roles get refs: `button`, `textfield`, `textarea`, `statictext`, `row`, `cell`, `checkbox`, `radiobutton`, `popupbutton`, `combobox`, `link`, `menuitem`, `menubutton`, `tab`, `slider`, `image`.

### `cu find --app X [--role R] [--title-contains S] [--title-equals S] [--value-contains S] [--first] [--raw]`
Predicate query. Faster + cheaper than `cu snapshot --limit 200 | grep ...`.

Filters AND-combine. Empty result is `ok:true` with `count:0` (not an error).
- `--first` — return one match instead of all
- `--raw` — print bare integer ref(s), one per line; exits 1 on no match. Designed for `$(...)` substitution: `REF=$(cu find --app X --role button --title-equals "OK" --first --raw)`.

### `cu nearest <x> <y> --app X [--max-distance N]`
Pixel → nearest interactive ref. For VLM agents that have visual coordinates from a screenshot.

Returns `match.ref`, `match.role`, `match.title`, `distance`, `inside` (bool — point falls inside the element). With `--max-distance N`, returns `match:null` if nothing's within N points — a sanity check for "did the VLM click on background or a real element".

### `cu observe-region <x> <y> <w> <h> --app X [--mode intersect|center|inside]`
All interactive refs whose bbox is in / touches a rectangle. Narrower than `cu snapshot`, more flexible than `cu nearest`.
- `intersect` (default) — element bbox overlaps the rect at all (broadest)
- `center` — element center falls inside (filters big container noise)
- `inside` — element fully contained (strictest)

### `cu ocr [app]`
Vision OCR text recognition. On-device, no network. Returns matches with `text`, `x`, `y`, `width`, `height`, `confidence` (0–1).

Aggregate fields on the response:
- `min_confidence`, `mean_confidence`, `low_confidence_count` (matches < 0.5)
- `confidence_hint` — string when any match is below 0.5. Vision returns plausible-looking hallucinations in this range; verify visually before acting on a low-confidence match.

Best for apps with poor AX support (games, Qt, Java, custom-drawn UIs).

### `cu screenshot [app] [--path P] [--region "x,y WxH"] [--full]`
Window capture, silent (no activation). Cross-Space capable via ScreenCaptureKit. Region capture uses point coords (same space as snapshot `x`/`y`).

Output:
- `path` — PNG location (default `/tmp/cu-screenshot-<ts>.png`)
- `offset_x`, `offset_y`, `width`, `height` — for mapping back from pixel to window/screen coords (`screen = pixel/scale + offset`)
- `image_scale` — pixel/point ratio (typically 2.0 on Retina)
- Capture-protected windows (`kCGWindowSharingState=0`) refuse upfront with a structured error — the OS-level opt-out cannot be bypassed.

Flags:
- `--full` — entire screen (all monitors) instead of single window
- `--region "x,y WxH"` — only that screen rectangle (cheap visual verification: ~150 tokens vs ~1500 for full window). **Overrides `--full` and the app argument.**

### `cu wait <condition> [--app X] [--timeout 10] [--limit N]`
Poll the AX tree until a condition is met. Returns `elapsed_ms` and `matched`.

Conditions:
- `--text "..."` — any element contains this text
- `--ref N` — element ref N exists
- `--gone N` — element ref N disappears
- `--new-window` — a new window appeared (sheet, compose, dialog)
- `--modal` — a modal/sheet is now frontmost
- `--focused-changed` — `AXFocusedUIElement` changed
- `--app-running` — the app finished launching

Prefer text/window/modal conditions over `--ref`/`--gone` — refs are unstable across UI changes.

---

## Action

All action commands accept `--app <Name>` for PID-targeted delivery (cursor stays, frontmost stays). Without `--app`, they go through the global HID tap. See `references/method_field.md` for routing details.

All action commands auto-attach a fresh `snapshot` to the response; suppress with `--no-snapshot`. They also report `method`, `confidence`, and `settle_ms` (capped at 500ms via single-shot AXObserver).

### `cu click <ref|x y> [--app X] [--ax-path P] [--text "..." [--index N] [--region "x,y WxH"]] [--right|--double-click] [--shift|--cmd|--alt] [--no-verify] [--no-snapshot] [--allow-global]`
Click by ref, screen coordinates, on-screen text (OCR), or stable `--ax-path` selector.

Verify is **on by default** — the response includes `verified` (bool), `verify_diff` (`{added, changed, removed}` element counts), and `verify_advice` (string with concrete recovery steps when `verified:false`).

Flags:
- `<ref>` — click by ref from snapshot. Runs the 15-step AX action chain (AXPress / AXConfirm / AXOpen / ancestor walks / coord-derived ref) before falling back to CGEvent.
- `<x> <y>` — click screen coordinates (CGEvent path).
- `--ax-path "<path>"` — stable selector that survives ref renumbering. Use for multi-step flows.
- `--text "Submit" [--index 2]` — find text via OCR and click it. Useful when AX is sparse.
- `--region "x,y WxH"` — restrict `--text` matching to OCR text whose center is inside this screen rectangle (in points). Disambiguates same-label-multi-pane (e.g. sidebar vs main pane).
- `--right` — right-click. Always CGEvent (skips AX action chain).
- `--double-click` — open files, select words.
- `--shift` / `--cmd` / `--alt` — modifiers.
- `--no-verify` — skip the AX-diff check (~50–150ms faster). Use only on known-reliable targets in bulk.
- `--allow-global` — last-resort flag to allow global-tap delivery when `--app` resolution fails. Avoid; bash-interval focus drift will land the click on the wrong window.

### `cu type <text> [--app X] [--paste|--no-paste] [--allow-global] [--no-snapshot]`
Type text into the focused element. Default routing is unicode CGEvent (`CGEventKeyboardSetUnicodeString`) — IME-bypass, no clipboard touch.

Auto-routes through clipboard paste when:
- text contains CJK / emoji / other non-ASCII that CEF apps drop, OR
- target app is on the chat-app list (WeChat, Slack, Discord, Telegram, Lark/Feishu, QQ/TIM, DingTalk, WhatsApp, Signal).

When auto-routed, response carries `paste_reason` (e.g. `"contains CJK"` / `"chat app target"`).

- `--paste` — force paste mode regardless of detection.
- `--no-paste` — force unicode-event delivery even when auto-paste would trigger.
- `--allow-global` — skip the terminal/IDE safety check (see below).

**Refused (without `--app`) when frontmost is a terminal/IDE** — same safety check as `cu key`: a stray keystroke would land in the agent's own shell. Always pass `--app` for normal use.

### `cu key <combo> [--app X] [--allow-global] [--no-snapshot]`
Send a keyboard shortcut. Modifiers: `cmd`, `shift`, `ctrl`, `alt` (option). Keys: `a-z`, `0-9`, `enter`, `tab`, `space`, `escape`, `delete`, `up/down/left/right`, `f1`–`f12`, `minus`, `equal`, etc.

**Refused when frontmost is a terminal/IDE** — the safety check exists because a stray `cmd+w` would close the agent's own shell. Pass `--allow-global` to override; don't unless you know what you're doing.

### `cu set-value <ref|--ax-path> "text" --app X [--no-snapshot]`
Write text directly into an AX field via `AXValue` setter. **No focus needed, no IME involved, no clipboard touched.** Best path for filling textfields when supported. Returns `method: "ax-set-value"`.

When it fails (Electron / non-AX field), the response says so — fall back to `cu click <ref>; cu type "..."`.

### `cu perform <ref|--ax-path> <AXAction> --app X [--no-snapshot]`
Invoke a named AX action. Common actions: `AXPress`, `AXShowMenu` (right-click equivalent), `AXIncrement` / `AXDecrement` (sliders/steppers), `AXScrollToVisible`, `AXConfirm`, `AXCancel`, `AXOpen`, `AXPick`, `AXRaise`. The available actions for any element are reported by `cu why <ref>`.

### `cu scroll <up|down|left|right> [amount] --x X --y Y [--app A]`
Scroll N lines (default 3) at a screen position.

### `cu hover <x> <y> [--app A]`
Move the mouse to a coordinate. Triggers tooltips and hover menus. Cursor moves by design even with `--app`.

### `cu drag <x1> <y1> <x2> <y2> [--shift|--cmd|--alt] [--app A]`
Drag with 10-step interpolation. `--shift` = extend selection, `--alt` = copy in Finder. Guarantees `mouseUp` even if intermediate steps fail. Cursor moves by design even with `--app`.

---

## Scripting

### `cu tell <app> '<AppleScript>'`
Run AppleScript against the app. Auto-wraps your expression in `tell application "<app>" ... end tell`. Multi-line scripts work — pass them inside the single-quoted argument.

Behavior:
- Auto-launches the app if not already running.
- App name is escaped against AppleScript injection.
- Uses `osascript -ss` for unambiguous structured output.
- Default timeout 10s.

When to use: any time the target app has the `S` flag in `cu apps`. Reading data and bulk operations are dramatically cheaper via AppleScript than via UI automation.

---

## System

### `cu defaults read <domain> [key]` / `cu defaults write <domain> <key> <value>`
Read or write macOS preferences via the `defaults` system. Bypasses System Settings UI entirely.
- `cu defaults read com.apple.dock` — entire domain
- `cu defaults read com.apple.dock autohide` — single key
- `cu defaults write com.apple.dock autohide -bool true` — write; `killall Dock` to apply
- Common domains: `com.apple.dock`, `com.apple.finder`, `NSGlobalDomain`

### `cu window <list|move|resize|focus|minimize|unminimize|close>`
- `cu window list [--app X]` — every visible window or filter by app. Returns `app`, `index`, `title`, `x`, `y`, `width`, `height`, `minimized`, `focused`.
- `cu window move <x> <y> --app X [--window N]` — N defaults to 1 (frontmost).
- `cu window resize <w> <h> --app X [--window N]`
- `cu window focus --app X` — bring app/window to front. The one place where `cu` deliberately does take focus. `method` is `ax-raise` when AX `AXRaise` succeeds (preferred — no app-level activation), else `applescript-frontmost` fallback.
- `cu window minimize/unminimize/close --app X [--window N]`

### `cu launch <name|bundleId> [--no-wait] [--timeout 10]`
Launch app and wait for first AX-ready window (avoids the empty-tree problem on cold starts). `--no-wait` for spawn-and-go. Returns `ready_in_ms`. Both name (`"TextEdit"`) and bundle ID (`"com.apple.TextEdit"`) work.

### `cu warm <app>`
Pay the 200–500ms first-AX-call cost up front for an already-running app the user opened manually. Useful at the very start of a task to make the first `cu state` / `cu snapshot` snappy.

### `cu why <ref> --app X`
Diagnose why a click / perform / set-value didn't take. Returns the element's `enabled`, `in_bounds` (vs window frame), `actions[]` (available AX actions), and `advice` (text — e.g. *"element is offscreen, scroll first"* / *"element is disabled"* / *"role doesn't support AXPress, try AXShowMenu"*).

Reach for it whenever an action returned `ok:false` or `verified:false` and the cause isn't obvious.

---

## Universal output fields

Every JSON response includes:
- `ok` — bool, success
- `error` — string, only when `ok:false`. Always actionable: `"element [99] not found in AX tree (scanned 50 elements — try --limit 100)"`.

Most action responses additionally include:
- `method` — routing path: `ax-action` / `ax-set-value` / `ax-perform` (best — no cursor move) / `cgevent-pid` / `unicode-pid` / `key-pid` / `ocr-text-pid` (PID-targeted, non-disruptive) / `*-global` (global tap, disruptive — usually means `--app` was missing). Full table: `references/method_field.md`.
- `confidence` — `high` / `medium` / `low`. Coord-based and global-tap actions skew lower.
- `advice` — string, present only when not best-case (e.g. *"pass --app to keep cursor put"*).
- `settle_ms` — actual ms waited for AXObserver post-action settle (capped at 500ms).
- `snapshot` — fresh AX tree of the target app (suppress with `--no-snapshot`).

**Reliability advisory strings — always read them when present:**
| field | command | meaning |
|---|---|---|
| `verify_advice` | click | action ran but AX tree didn't change → follow recovery steps |
| `truncation_hint` | snapshot, state | `--limit` was hit → re-run with larger limit |
| `confidence_hint` | ocr | a match is below 0.5 → Vision may have hallucinated |
| `paste_reason` | type | text was auto-routed via clipboard (CJK or chat-app target) |
| `screenshot_error` | state, screenshot | capture refused by `kCGWindowSharingState=0` — AX tree still works |

These exist because boolean flags (`verified:true`, `truncated:true`) are easy to skim past. Strings are not.
