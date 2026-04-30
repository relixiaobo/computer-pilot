---
name: computer-pilot
description: >
  Control the macOS desktop via the `cu` CLI tool. Activate whenever a task
  asks Claude to drive a macOS GUI app on the user's behalf — clicking
  buttons, filling forms, reading screen state, controlling Finder / Mail /
  Calendar / Notes / Messages / WeChat, automating menu bars, taking
  window screenshots, or reading on-screen text via OCR. Three-tier
  control: AppleScript (scriptable apps) → AX tree + CGEvent
  (non-scriptable) → OCR + screenshot (fallback). Do NOT activate for
  text-only file edits inside an editor, even when the editor is "an app".
---

# Computer Pilot

Control macOS desktop applications via the `cu` CLI. Three tiers of control:
1. **AppleScript** (`cu tell`) — direct app data access for scriptable apps (fastest, most reliable)
2. **AX tree** (`cu snapshot` + `cu click`) — UI element automation for any app
3. **OCR/Screenshot** — visual fallback for apps with poor AX support

## Prerequisites

- `cu` binary must be installed (Rust, single binary)
- Run `cu setup` to check Accessibility + Screen Recording + Automation permissions

## Choose the Right Approach

```bash
cu apps                              # S flag = scriptable → use cu tell
```

**Scriptable app (has S flag)?** → Use `cu tell` with AppleScript. Faster, more reliable, reads/writes app data directly.

**Non-scriptable app?** → Use `cu menu` to discover capabilities, then `cu snapshot` → `cu click` for interaction.

**System settings?** → Use `cu defaults read/write` to change preferences directly (no System Settings UI needed).

## Decision Tree (which command to use)

When you have a goal, walk down this tree before you write any commands:

```
Need to act on the system?
├─ App not yet running?            → cu launch <name|bundleId>      (D6: waits for window-ready)
├─ Starting a task on a specific app?
│                                  → cu state <app>                  (snapshot + screenshot + windows + frontmost in one call)
├─ Read/write app data?
│   ├─ S flag in `cu apps`?        → cu tell <App> '<AppleScript>'
│   └─ otherwise                   → cu snapshot → cu set-value / cu type
├─ Click something?
│   ├─ Know its ref (just snapshotted)? → cu click <ref> --app <X>          (verify is on by default — read `verified` + `verify_advice`)
│   ├─ Multi-step / UI mutates?         → cu click --ax-path "<path>" --app <X>   (A2: stable across snapshots)
│   ├─ Know its text/role?              → cu find --first --raw --role <R> --title-contains <S> --app <X> | xargs cu click --app <X>
│   ├─ Know visual coords (VLM)?        → cu nearest <x> <y> --app <X>   (or cu click <x> <y>)
│   ├─ See it but no AX/text?           → cu click --text "Submit" --app <X>   (OCR-driven)
│   └─ Need extra speed (bulk clicks)?  → cu click <ref> --app <X> --no-verify
├─ Fill a textfield?
│   ├─ AX field, just snapshotted?     → cu set-value <ref> "..."           (no focus needed)
│   ├─ AX field, multi-step flow?      → cu set-value --ax-path "<path>" "..."   (selector survives UI churn)
│   ├─ Chat app or CJK/emoji?          → cu type "..." --app <X>            (auto-routes via paste — see paste_reason in output)
│   └─ Electron / non-AX?              → cu click <ref>; cu type "..."
├─ Read screen?
│   ├─ Tree layout (cheap)?        → cu snapshot --limit N
│   ├─ Only what changed?          → cu snapshot --diff
│   ├─ See a region (VLM)?         → cu screenshot --region "x,y WxH"
│   └─ Pixel + ref labels (VLM)?   → cu snapshot --annotated --output p.png
├─ Wait for state?
│   ├─ Text appears?               → cu wait --text "..." --app <X>
│   ├─ New window opens?           → cu wait --new-window --app <X>
│   ├─ Modal/sheet appears?        → cu wait --modal --app <X>
│   ├─ Focus moves?                → cu wait --focused-changed --app <X>
│   └─ Element disappears?         → cu wait --gone <ref> --app <X>
├─ System preferences?             → cu defaults read/write <domain> <key>
└─ Click/perform returned ok=false (or the UI didn't change)?
                                   → cu why <ref> --app <X>   (B7: structured "what's wrong with this ref" report)
```

**Hard rules** (numbered — read in order):

1. **Always start with `cu state <app>`.** One call returns snapshot + screenshot + windows + frontmost. Do NOT start with `cu setup` then `cu apps` then `cu snapshot` — that's three round-trips for what `cu state` gives in one. Reserve `cu setup` for when permissions are actually broken; reserve `cu apps` for when you genuinely don't know what's running.

2. **Every action command needs `--app <Name>`.** Applies to `click`, `key`, `type`, `scroll`, `hover`, `drag`, `set-value`, `perform`. With `--app`, events are PID-targeted — cursor stays put, frontmost stays put. Without `--app`, events go through the global HID tap and hit whatever is frontmost (which may have shifted between bash invocations). `cu key` / `cu type` refuse outright when frontmost is a terminal/IDE.

3. **Refs are ephemeral, axPaths are stable.** Refs refresh with every snapshot. For multi-step flows, save the `axPath` from the first snapshot and pass `--ax-path` to subsequent click/set-value/perform calls.

4. **Read the auto-attached `snapshot`** (and `settle_ms`) after every action instead of calling `cu snapshot` again.

5. **`cu click` verify is ON by default.** Pre/post AX diff catches sandboxed/Electron apps that silently drop PID-targeted CGEvents — the #1 "ok=true but UI didn't change" failure. Always read `verified` in the response.
   - When `verified=false`: read `verify_advice`. The right recovery is **another cu primitive with `--app`**, not a global-tap fallback. Concretely: try the action again via a different cu method (e.g., `--ax-path` instead of coords; `cu set-value` instead of `click + type`; `cu perform <ref> AXPress` instead of coord click). If the AX tree is too sparse for ref-based action, **first** focus the window via a single AppleScript (`osascript -e 'tell application "X" to activate'`) and retry the cu command — but stay on `--app`-targeted cu, **never** drop to `--allow-global` mid-flow.
   - Use `--no-verify` only on bulk operations against a known-reliable target.

6. **`cu type` auto-routes through clipboard paste** when text contains CJK or target is a chat-app (WeChat, Slack, Discord, Telegram, Lark/Feishu, QQ/TIM, DingTalk, WhatsApp, Signal). When auto-routed, `paste_reason` appears in the JSON output. Force off with `--no-paste`. Force on with `--paste`.

7. **Read every `*_hint` / `*_reason` / `*_advice` / `*_error` string.** When cu attaches one of these, the result is degraded or auto-corrected and you need to react. Names to watch: `truncation_hint` (snapshot clipped — bigger `--limit`), `confidence_hint` (OCR has low-confidence matches — verify visually), `paste_reason` (type was auto-pasted), `verify_advice` (action ran but tree didn't change), `screenshot_error` (capture refused, usually `kCGWindowSharingState=0`).

8. **Window identity is unified across `cu snapshot` / `cu screenshot` / `cu click`.** All resolve via AX (`AXFocusedWindow` → `AXMainWindow` → `_AXUIElementGetWindow`), so what you see in the snapshot is what you'll capture and click on. Cross-Space windows work (ScreenCaptureKit primary path). `kCGWindowSharingState=0` apps refuse capture upfront with a structured error — drive those tasks via AX (`snapshot/find/click`), no visual verification.

## Anti-patterns — DO NOT do these

These are concrete dead-ends. They look like reasonable approaches; they're not. If you find yourself reaching for one, stop — there's a cu primitive that does it correctly.

| ❌ Don't | Why it fails | ✅ Do instead |
|---|---|---|
| `osascript -e 'tell ... keystroke "..."'` for typing | Drops CJK silently (no error). Bypasses cu's auto-paste, verify, and routing. Failure is invisible. | `cu type "..." --app <X>` |
| `pbcopy && osascript -e 'keystroke "v" using command down'` for paste | Reinventing what `cu type --paste` already does (and the auto-detection in #6 already triggers it). Loses `paste_reason` field. | `cu type "..." --app <X>` |
| `cu window focus --app X && cu click ... --allow-global` for sandboxed apps | The `&&` between bash calls lets focus drift back to the terminal. Click lands on the terminal, not the target. | Stay on cu primitives with `--app`. Try a different action method (ax-path, perform, set-value), not a different focus mode. |
| `cu click --allow-global` as a recovery for `verified: false` | `--allow-global` is global HID tap — every focus race in subsequent bash invocations affects it. | Retry via cu primitive with `--app`. If AX is sparse, single `osascript activate` then cu. |
| `cu setup && cu apps && cu snapshot <app>` to start a task | Three round-trips to learn what `cu state` returns in one. | `cu state <app>` |
| `cu snapshot <app>` first, then guess flag for `cu screenshot` | `cu state` already attaches the screenshot on the same window — no second resolve. | `cu state <app>` (the screenshot is in the response). |
| Chaining `cu` action commands via `&&` for multi-step flows | Each bash invocation has its own focus epoch. State changes between calls; the second cu call may target a different focus. | Issue them as separate steps so you can read each result before the next, and re-snapshot if the auto-attached snapshot shows the UI moved. |
| Treating `verified: true` as "the click did the right thing" | Verified only means the AX tree changed — not that it changed the way you wanted. | After any verified action, read the post-action snapshot to confirm the change matches your intent. |

## Scripting Workflow (preferred for scriptable apps)

```bash
cu apps                                          # 1. check S flag
cu sdef "App Name"                               # 2. discover available properties/commands
cu tell "App Name" 'get name of every calendar'  # 3. read/write data via AppleScript
```

### Examples

```bash
# Read data
cu tell Safari 'get URL of current tab of front window'
cu tell Finder 'get name of every item of front Finder window'
cu tell Notes 'get plaintext of note 1'
cu tell Reminders 'get name of every reminder whose completed is false'
cu tell "System Events" 'get dark mode of appearance preferences'

# Mail — read emails (use specific mailbox for speed, "inbox" is slow on large mailboxes)
cu tell Mail 'get subject of message 1 of inbox'
cu tell Mail 'get content of message 1 of inbox'                        # email body text
cu tell Mail 'get {subject, sender, date received} of messages 1 thru 5 of inbox'
cu tell Mail 'get content of message 1 of mailbox "INBOX" of account 1' # faster on large mailboxes

# Calendar — multi-line scripts work (passed via stdin)
cu tell Calendar 'set d to (current date) + (1 * days)
set hours of d to 10
set minutes of d to 0
set seconds of d to 0
make new event at end of events of first calendar with properties {summary:"Meeting", start date:d, end date:d + (1 * hours)}'

# Write data
cu tell Notes 'make new note with properties {name:"Title", body:"Content"}'
cu tell Reminders 'make new reminder with properties {name:"Buy milk"}'

# System control
cu tell "System Events" 'set dark mode of appearance preferences to true'
```

## AX Tree Workflow (for non-scriptable apps)

For apps WITHOUT the `S` flag in `cu apps`: go straight to UI automation. Do NOT try `cu tell` — it will waste steps.

```bash
cu snapshot "App Name" --limit 50    # 1. get UI elements with [ref] numbers
cu click 3 --app "App Name"          # 2. interact by ref
cu snapshot "App Name" --limit 50    # 3. verify result, get new refs
cu click 7 --app "App Name"          # 4. next action with updated refs
```

### Example: Calculator (not scriptable)
```bash
cu snapshot Calculator --limit 30    # See: [6] button "All Clear", [18] button "1", etc.
cu click 6 --app Calculator          # Clear
cu click 18 --app Calculator         # Press "1"
cu click 16 --app Calculator         # Press "6"
cu click 22 --app Calculator         # Press "0"
cu click 13 --app Calculator         # Press "Multiply"
cu click 12 --app Calculator         # Press "9"
cu click 24 --app Calculator         # Press "Equals"
# For scientific calculator: View menu → Scientific (or cmd+2)
cu key cmd+2 --app Calculator        # Switch to scientific mode
cu snapshot Calculator --limit 50    # Now shows sin, cos, tan, etc.
```

### VLM agents: tree + image in one atomic call

When the VLM needs to look at the actual UI (without ref overlays distracting it) AND act via refs in the same step, use `--with-screenshot`:

```bash
cu snapshot Mail --limit 50 --with-screenshot --output /tmp/m.png
# JSON: elements[] + window_frame + screenshot:/tmp/m.png + image_scale:2.0
```

Both the tree and the image are captured in the same call → guaranteed to reflect the same UI instant (no race between two separate `cu` invocations). Combine with `--diff` to get only changed elements + a fresh image.

If you also want refs drawn on the image, use `--annotated` instead (it supersedes `--with-screenshot`).

### VLM agents: cheap visual verification

To check "did that button turn grey?" or "is the modal gone?", don't re-screenshot the whole window. Use `--region` to grab only the area you care about:

```bash
cu screenshot --region "480,200 400x300" --path /tmp/ck.png  # 200×100 area
# ~150 tokens for the VLM vs ~1500 for a full 1920×1200 window
```

Region coords are in **points** (same space as snapshot element x/y). Output PNG is in pixels (Retina is 2×); response always echoes `offset_x/offset_y/width/height` so you can map back.

### VLM agents: region → candidate refs

When the VLM has narrowed the area of interest to a rectangle (a dialog, a list area, a toolbar), `cu observe-region` returns the candidate set inside it — narrower than `cu snapshot`, more flexible than `cu nearest`:

```bash
cu observe-region 480 200 400 300 --app Mail              # intersect (default)
cu observe-region 480 200 400 300 --app Mail --mode center  # less noise
cu observe-region 480 200 400 300 --app Mail --mode inside  # strictest
```

`--mode` choices:
- `intersect` — element bbox overlaps the rect at all (broadest)
- `center` — element center point falls inside (filters big container noise)
- `inside` — element fully contained (strictest)

Combine with `cu find` filters by piping through jq if you need role/title narrowing on top.

### VLM agents: visual coords → ref

When the VLM has identified WHERE on screen something is but doesn't have a ref, `cu nearest <x> <y>` translates the pixel into the closest interactive element:

```bash
cu nearest 480 320 --app Mail
# → {"match":{"ref":12,"role":"button","title":"Send","distance":0.0,"inside":true}}
REF=$(cu nearest 480 320 --app Mail | jq -r .match.ref)
cu click "$REF" --app Mail
```

`distance:0 inside:true` means the point falls inside the element. With `--max-distance N`, returns `match:null` if nothing's within N points — useful for "did the VLM click on background or a real element" sanity checks.

### VLM agents: look at the screen, click by ref

When the agent has vision, the highest-leverage flow is `--annotated`: cu draws each ref's bounding box + number directly on a window screenshot. The agent looks at the image, identifies the right element by visual cues (color, position, neighboring text), then clicks by ref — no coordinate guessing.

```bash
cu snapshot Mail --limit 50 --annotated --output /tmp/mail.png
# JSON includes: "annotated_screenshot": "/tmp/mail.png", "image_scale": 2.0
# (image_scale = pixel/point ratio, typically 2.0 on Retina)
```

The agent then opens `/tmp/mail.png`, sees boxes labeled `[3]`, `[7]`, `[12]`, picks one by visual identification, and runs `cu click 12 --app Mail`. This sidesteps the failure mode of "VLM picks coordinates that drift after a re-render".

### Cheap re-snapshots between actions

For multi-step flows, `cu snapshot --diff` returns only the elements that changed since the last snapshot of this app — usually a fraction of the full tree. The cache is per-pid at `/tmp/cu-snapshot-cache/<pid>.json`.

```bash
cu snapshot Mail --limit 100              # baseline (caches it)
cu click 12 --app Mail --no-snapshot       # do something
cu snapshot Mail --limit 100 --diff        # → +N ~M -K, usually << 100
```

The first `--diff` call (no cache) returns the full snapshot with `first_snapshot:true`. Identity is `(role, round(x), round(y))` — robust to ref re-numbering, sensitive to window movement (a moved window will show all elements as removed+added).

### Targeted query (preferred over `snapshot` + grep)

When you know what you're looking for, `cu find` is faster and cheaper than `cu snapshot --limit 200 | grep ...`:

```bash
# Find one element and act on it in two steps
cu find --app Mail --role textfield --title-contains "Subject" --first
# → {"match":{"ref":7,"role":"textfield",...},"count":1}
cu set-value 7 "Hello" --app Mail

# Or in one pipe
REF=$(cu find --app Safari --role button --title-equals "Reload" --first | jq -r .match.ref)
cu click "$REF" --app Safari

# Even simpler with --raw — bare integer, no jq needed
REF=$(cu find --app Safari --role button --title-equals "Reload" --first --raw)
cu click "$REF" --app Safari
```

`--raw` prints bare ref integers (one per line), exits 1 on no match — designed for `$(...)` substitution and shell pipelines.

Filters are AND-combined: `--role`, `--title-contains` (case-insensitive), `--title-equals`, `--value-contains`. Empty result is `ok:true` with `count:0` — not an error.

### Text-based click (for UI not in AX tree)
When elements don't appear in `cu snapshot`, use OCR text click:
```bash
cu click --text "Edit Widgets" --app "System Settings"    # finds text via OCR, clicks it
cu click --text "Submit" --app Safari --index 2            # click 2nd match
```

## Understanding Snapshots

`cu snapshot` returns the AX tree — a structured list of interactive UI elements:

```
[1] button "Back" (10,40 30x24)
[2] textfield "Search" (100,40 200x24)
[3] statictext "Favorites" (10,100 80x16)
[4] row "Documents" (10,120 300x20)
```

Each element has: `[ref]` number, role, title/value, position, size.
Use the `[ref]` number with `cu click <ref>` to interact.
**Refs change after every action** — always re-snapshot before clicking.

## Commands cheat sheet

The 17 commands you'll reach for most (out of 27 total). **Full per-flag reference (every command, every flag, every output field):** `references/commands.md`.

| Command | One-line purpose |
|---|---|
| `cu state <app>` | **First call.** Tree + windows + screenshot + frontmost in one round-trip |
| `cu apps` | List running apps (`*`=frontmost, `S`=scriptable) |
| `cu menu <app>` | Dump every menu-bar item (works on any app, scriptable or not) |
| `cu launch <name\|bundleId>` | Launch + wait for first AX-ready window (or `--no-wait`) |
| `cu snapshot <app> [--limit N] [--diff] [--annotated]` | AX tree; `--diff` = changes since last; `--annotated` = PNG with refs drawn |
| `cu find --app X --role R --title-contains S [--first --raw]` | Predicate query (faster + cheaper than snapshot+grep) |
| `cu click <ref> --app X` | Click + verify by default; read `verified` and `verify_advice` |
| `cu set-value <ref\|--ax-path> "text" --app X` | Write into an AX field — no focus, no IME |
| `cu perform <ref\|--ax-path> <AXAction> --app X` | Invoke a named AX action (`AXPress`, `AXShowMenu`, …) |
| `cu type "text" --app X` | Type; auto-routes via paste for CJK / chat apps (`paste_reason` in output) |
| `cu key <combo> --app X` | Keyboard shortcut; refused when frontmost is a terminal/IDE |
| `cu screenshot <app> [--path p.png] [--region "x,y WxH"]` | Window or rectangle capture (silent, no activation) |
| `cu ocr <app>` | Vision OCR for apps with sparse AX |
| `cu wait --text "..." --app X --timeout 10` | Poll until text / window / modal / focus condition |
| `cu tell <app> '<AppleScript>'` | Run AppleScript against the app (read/write data directly) |
| `cu why <ref> --app X` | Diagnose why a click/perform/set-value didn't take |
| `cu defaults read/write <domain> <key>` | Read/write macOS preferences without opening Settings |

Window management (`cu window list/move/resize/focus/minimize/close`), `cu sdef`, `cu warm`, `cu nearest`, `cu observe-region`, `cu setup`, `cu examples` — covered in `references/commands.md`.

## Perception Strategy

Cheapest first: **`cu snapshot`** (AX tree, lowest tokens) → **`cu ocr`** (Vision, for apps with sparse AX — games, Qt, Java) → **`cu screenshot`** (image, when you need vision).

## Cookbook (high-frequency recipes)

Each recipe is the shortest correct sequence. Copy, swap names, run.

### 1. Start a task on an app — always do this first
```bash
cu state Mail                        # tree + windows + screenshot + frontmost in one call
cu state Mail --no-screenshot        # same, ~50ms faster (skip capture)
```
This is the canonical first move. Reading the response gives you everything `cu apps` + `cu window list` + `cu snapshot` + `cu screenshot` would give in four separate round-trips. If the response carries `screenshot_error`, the app is capture-protected — proceed via AX (`elements`) without visual verification.

### 2. Launch an app and wait until usable
```bash
cu launch TextEdit                 # waits up to 10s for AX-reported window
cu launch com.apple.Calculator     # bundle id form
cu launch Mail --no-wait           # spawn-and-go
```
Avoids the empty-AX-tree problem on cold starts. Returns `ready_in_ms` so you know how long it took. For an app the user opened manually, run `cu warm <app>` to pay the first-AX-call cost up front.

### 3. Read or write app data via AppleScript (preferred for scriptable apps)
```bash
cu apps                                                          # check S flag
cu sdef Calendar                                                 # discover schema
cu tell Calendar 'get summary of every event of first calendar'
cu tell Notes 'make new note with properties {name:"Title", body:"Content"}'
```
If the app has the `S` flag, `cu tell` will almost always be faster and more reliable than UI automation.

### 4. Fill a textfield or click a button you can name
```bash
# Fill — no focus, no IME
REF=$(cu find --app Mail --role textfield --title-contains Subject --first --raw)
cu set-value "$REF" "Quarterly review" --app Mail

# Click — by exact label
REF=$(cu find --app Safari --role button --title-equals Reload --first --raw)
cu click "$REF" --app Safari
```
If `set-value` fails (Electron / non-AX field): `cu click "$REF"; cu type "..."`. If the button isn't in the AX tree at all: `cu click --text "Reload" --app Safari` (OCR-driven).

### 5. Multi-step flow that mutates the UI — capture axPath once
```bash
SNAP=$(cu snapshot Mail --limit 100)
SUBJECT=$(echo "$SNAP" | jq -r '.elements[] | select(.role=="textfield" and (.title // "") | contains("Subject")) | .axPath')
SEND=$(echo "$SNAP" | jq -r '.elements[] | select(.role=="button" and (.title // "") == "Send") | .axPath')

cu set-value --ax-path "$SUBJECT" "Quarterly review" --app Mail
cu click     --ax-path "$SEND"    --app Mail
```
Use `--ax-path` whenever a step opens a sheet / expands a section / triggers a re-render. Refs renumber; axPaths survive.

### 6. VLM agent: screen → ref → click
```bash
cu snapshot Mail --limit 50 --annotated --output /tmp/m.png  # boxes drawn for each ref
# Agent looks at /tmp/m.png, identifies element by visual cues, then:
cu click 12 --app Mail

# Or: VLM has pixel coords from a separate screenshot — translate to nearest ref
REF=$(cu nearest 480 240 --app Mail | jq -r .match.ref)
cu click "$REF" --app Mail

# Or: scope to a rectangle (dialog / panel) instead of the whole window
cu observe-region 480 200 400 300 --app Mail --mode center

# Cheap visual re-check after acting (5–10× smaller than full window)
cu screenshot --region "480,200 400x300" --path /tmp/check.png
```
`--annotated` is the highest-leverage flow when the model has vision: it sees the UI with refs already labeled, so coordinate guessing never enters the loop.

### 7. Wait for something to happen
```bash
cu wait --new-window      --app Mail    --timeout 5   # sheet / compose window
cu wait --modal           --app Finder  --timeout 5   # save / replace dialog
cu wait --focused-changed --app Safari  --timeout 5   # focus moved to next field
cu wait --text "Saved"    --app TextEdit --timeout 10 # text appeared in the tree
```
Prefer text/window/modal conditions over `--ref` / `--gone` (refs are unstable).

### 8. Send a message in a chat / IM app (WeChat, Messages, Slack, Telegram, Lark)
Chat apps trip three landmines simultaneously. **cu auto-handles all three** — your job is to read the advisory fields it surfaces.

1. **Partly sandboxed** — PID-targeted CGEvents may be silently dropped. `cu click` verifies by default; watch for `verified: false` + `verify_advice`.
2. **Rich-text editors drop leading unicode code units** — `cu type` auto-routes through clipboard paste for CJK content or chat-app targets. Watch for `paste_reason`.
3. **Some opt out of screen capture** (`kCGWindowSharingState=0` — WeChat, parts of Office MAS) — `cu state` / `cu screenshot` return `screenshot_error`; AX tree still works.

```bash
cu state WeChat                                      # tree + windows + (maybe) screenshot_error
INPUT=$(cu find --app WeChat --role textarea --first --raw)
cu click "$INPUT" --app WeChat                       # read `verified` + `verify_advice`
cu type  "你好，这是来自 cu 的消息" --app WeChat       # see `paste_reason` in output
cu key   enter --app WeChat                          # send (some chat apps want cmd+enter — try enter first)
```

**Recovery when `verified: false`:** stay on `--app`-targeted cu primitives. Try ref-based `--ax-path` click → `cu perform <ref> AXPress` → if AX is too sparse, single `osascript -e 'tell application "X" to activate'` then retry the **same** `cu click ... --app X`. **Never** drop to `--allow-global` — bash-interval focus drift will route the next click to your terminal.

**Recovery when `screenshot_error` appears:** that's the OS, not a cu bug. Drive the task with AX (`snapshot` / `find` / `click`); accept there's no visual verification path.

## Focus Model (why `--app` matters)

With `--app`: events go to the target app via `CGEventPostToPid` — cursor stays, frontmost stays, IME bypassed. Without `--app`: events flow through the global HID tap and hit whatever is frontmost (which can drift between bash invocations).

Every action response carries a `method` field documenting routing — `ax-action` / `ax-set-value` / `ax-perform` are best (no cursor move), `*-pid` are pid-targeted (non-disruptive), `*-global` are global tap (disruptive — usually means `--app` was missing).

Full method-value table and known limitations (drag/hover always move cursor; some MAS sandbox builds drop PID-targeted events): see `references/method_field.md`.

## Output Format (digest)

When piped, every command returns JSON; pass `--human` to force readable output. Snapshot elements carry both `ref` (ephemeral) and `axPath` (stable selector — pass via `--ax-path`). Action responses auto-attach a fresh `snapshot` (skip with `--no-snapshot`) plus a `method` routing field.

**Always read these advisory strings when present** — they mean the result is degraded or auto-corrected, and the agent must react:

| field | command | what it means |
|---|---|---|
| `verify_advice` | click | action ran but AX tree didn't change → follow the recovery steps |
| `truncation_hint` | snapshot | `--limit` was hit → re-run with larger limit |
| `confidence_hint` | ocr | a match is below 0.5 → Vision may have hallucinated; verify visually |
| `paste_reason` | type | text was auto-routed via clipboard (CJK or chat app) |
| `screenshot_error` | state, screenshot | capture refused (usually `kCGWindowSharingState=0`) — AX tree still works |

Boolean flags like `verified`, `truncated`, `ok` matter too, but they're easy to skim past — these advisory strings exist precisely so you don't.

For the full per-field catalog (every field on every command), see `references/commands.md`.

## Tips

- For Chrome/web tasks, prefer `bp` (browser-pilot) over `cu` — DOM-level precision beats AX tree for web content
- For desktop app tasks, `cu` is the right tool — AX tree provides element refs that Chrome CDP can't access
- If `cu snapshot` returns sparse results, try `cu ocr` (Vision OCR) or `cu screenshot` (visual fallback)
- Use `cu wait --text "X"` after actions that trigger loading or transitions
- Clipboard: use `pbcopy`/`pbpaste` directly, no need for `cu` wrapper
