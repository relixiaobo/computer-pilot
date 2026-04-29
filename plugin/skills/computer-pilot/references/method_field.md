# `method` field — routing reference

Every `cu` action response carries a `method` field that documents which delivery path was taken. Read it when debugging "did this disrupt the user" or "why didn't this take effect."

## Why it exists

`cu`'s non-disruption guarantee comes from **per-process CGEvent delivery**: when `--app <Name>` is given, every event is posted via `CGEventPostToPid` to the resolved pid instead of through the global HID tap. Cursor stays put, frontmost stays frontmost, IME state is bypassed.

Without `--app`, events go through the global HID tap and hit whatever happens to be frontmost — which can drift between bash invocations.

## Method values

| `method` | path taken | disruptive? | notes |
|---|---|---|---|
| `ax-action` | AX native action (`AXPress`, `AXConfirm`, `AXOpen`) | no | Best — no cursor move at all. Used when click/perform can target a ref. |
| `ax-set-value` | `AXValue` setter (`cu set-value`) | no | No focus, no IME, no clipboard. Best path for textfield fills. |
| `ax-perform` | named AX action (`cu perform`) | no | Direct call to whatever AX action the element exposes. |
| `cgevent-pid` | `CGEventPostToPid` mouse click | no | Cursor stays. Used by `cu click` when AX action chain doesn't fit. |
| `unicode-pid` | `CGEventPostToPid` unicode keystroke | no | Used by `cu type` default routing (non-CJK, non-chat-app). |
| `unicode-paste-pid` | clipboard paste via PID | no | Used by `cu type` when auto-paste triggers — CJK / chat-app target. Look for `paste_reason`. |
| `key-pid` | `CGEventPostToPid` key combo | no | Used by `cu key` when `--app` is given. |
| `ocr-text-pid` | OCR-located coord click via PID | no | Used by `cu click --text` when `--app` is given. |
| `cgevent-global` | global HID tap mouse click | **yes** | Cursor warps. Means `--app` was missing or resolution failed. |
| `unicode-global` | global HID tap unicode | **yes** | Frontmost-app delivery. |
| `key-global` | global HID tap key combo | **yes** | Refused for terminal/IDE-frontmost unless `--allow-global`. |
| `ocr-text-global` | OCR-located click via global tap | **yes** | Same as `cgevent-global`, just OCR-coord-derived. |

## Debugging

**If you see a `*-global` method**, `--app` was either missing or didn't resolve. Add `--app <Name>` and try again. The exception is `--allow-global` (passed deliberately).

**If you see `cgevent-pid` / `unicode-pid` but `verified:false`**, the target app is sandboxed and silently dropping PID-targeted events — this is the #1 cause of "ok:true but UI didn't change." Recovery: re-snapshot, retry via a different cu primitive (`--ax-path`, `cu perform <ref> AXPress`, `cu set-value`), or single `osascript activate` then retry the same `cu click ... --app X`. Never fall back to `--allow-global`.

## Known limitations of pid-targeted delivery

- **`cu drag` and `cu hover` move the cursor by design** — pid-targeting suppresses focus theft but the cursor still moves to the target coordinates. There's no way around this; both commands are inherently visual.
- **Some MAS-sandboxed apps ignore PID-targeted events.** Symptom: action returns `ok:true` but the UI doesn't update. `cu click` catches this via verify (returns `verified:false` + `verify_advice`). Workaround: focus the app first via `osascript -e 'tell application "X" to activate'`, then retry the cu command — stay on `--app`-targeted cu, don't drop to global tap.
- **The `EventSource` RAII wrapper** in `src/mouse.rs` / `src/key.rs` creates a `kCGEventSourceStateCombinedSessionState` (=0) source when targeted, so PID events do not collide with the user's real HID stream. Without `--app`, the source is null (default global source).

## Implementation

`src/main.rs` resolves `--app` to a pid up front and passes it down to `mouse::*` / `key::*` for every action command (`click`, `type`, `key`, `scroll`, `hover`, `drag`, `set-value`, `perform`). The pid is what makes `*-pid` methods possible.

`cu type` uses `CGEventKeyboardSetUnicodeString` with `virtual_key=0` — it injects UTF-16 code units directly per CGEvent, bypassing IME. No pbcopy/pbpaste round-trip on the default path. Auto-paste only kicks in when CJK / chat-app detection triggers (see `cu type --help`).
