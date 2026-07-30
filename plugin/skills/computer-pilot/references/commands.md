# CLI Command Reference

Use this reference for command selection and common flags. Run
`cu <command> --help` for the authoritative syntax of the installed version.
All commands emit JSON when piped; use `--human` only for interactive reading.

## Global Options

```text
--client-key <key>       Stable logical Agent identity
--request-id <id>        Idempotency key for command recovery
--timeout <milliseconds> Broker acceptance-to-completion deadline
--json                   Force machine output
--human                  Force human output
```

Environment equivalents:

```text
COMPUTER_PILOT_CLIENT_KEY
COMPUTER_PILOT_OUTPUT_DIR
```

The output directory must be absolute. File commands refuse overwrite and
symlink traversal.

Every UI `<app>`/`--app <app>` value accepts a unique application name, a
unique bundle identifier, or `pid:<PID>`. Name and bundle selectors fail with
`ambiguous_target` when multiple processes match. Use the exact `selector`
returned by `cu apps` for development/production copies or other duplicate
instances.

## Discover

### `cu setup`

Report version, Accessibility, Screen Recording, readiness, capture-protected
running apps, and per-target Automation semantics. Run only after install or a
permission failure.

### `cu apps`

List running applications with `name`, `pid`, reusable `selector`, `bundle_id`,
`bundle_path`, `active`, `scriptable`, and optional `sdef_classes`. Application
identity comes from NSWorkspace and does not request Automation permission.

### `cu menu <app>`

List menu bar items and enabled state. This currently uses System Events and
may require per-target Automation access.

### `cu sdef <app>`

Parse an app's scripting dictionary. Use before `cu tell` when the AppleScript
object model is unfamiliar.

### `cu examples [topic]`

Print built-in recipes from the installed CLI.

## Observe

### `cu state <app> [--limit N] [--no-screenshot] [--output /abs/file.png]`

Canonical first call. Returns app/PID, windows, displays, AX elements,
frontmost state, `observation_id`, and normally a screenshot.

File fields:

- `screenshot`: legacy absolute path.
- `screenshot_file`: `{path,mime,bytes,width,height,scale}`.
- `image_scale`: legacy pixel-to-point scale.
- `screenshot_error`: capture failed while AX state remains usable.

### `cu snapshot [app] [--limit N] [--diff] [--annotated] [--with-screenshot] [--output /abs/file.png]`

Return interactive AX elements. Each element includes an ephemeral `ref`, role,
label/value, geometry, and often `axPath`. Top-level `observation_id` binds refs
to the current client and UI generation.

- `--diff`: compare with this client's private, TTL-bounded prior snapshot.
- `--annotated`: draw ref labels over a captured image.
- `--with-screenshot`: return tree and plain image in one call.
- `truncation_hint`: increase `--limit` before assuming an element is absent.
- `annotated_screenshot_file` / `screenshot_file`: structured file metadata.

### `cu find --app <app> [filters] [--first] [--raw]`

Query AX elements by `--role`, `--title-contains`, `--title-equals`, or
`--value-contains`. Filters combine with AND. Prefer normal JSON so the
`observation_id` is retained. `--raw` is intended only for controlled shell
workflows that also preserve Observation context.

### `cu nearest <x> <y> --app <app> [--max-distance N]`

Resolve a screen point to the closest interactive AX element. Inspect
`distance` and `inside` before acting.

### `cu observe-region <x> <y> <width> <height> --app <app> [--mode ...]`

Return interactive elements intersecting a screen-space rectangle. Modes are
`intersect`, `center`, and `inside`.

### `cu ocr [app]`

Run on-device Vision OCR. Inspect match confidence and `confidence_hint`.

### `cu screenshot [app] [--app <app>] [--path /abs/file.png] [--full] [--region <spec>]`

Capture an AX-selected window without activation, the full virtual desktop, or
a region. Returns `path`, `file:{path,mime,bytes,width,height,scale}`, mode, and
coordinate offsets where applicable. Window capture uses ScreenCaptureKit with
a CoreGraphics fallback and refuses capture-protected windows.

### `cu wait [condition] --app <app> [--timeout seconds] [--limit N]`

Poll until `--text`, `--ref`, `--gone`, `--new-window`, `--modal`,
`--focused-changed`, or `--app-running` succeeds. Prefer semantic text/window
conditions over ref conditions.

### `cu why <ref> --app <app>`

Explain target enabled state, bounds, supported AX actions, and recovery advice.

## Act

Always pass `--app`. Ref actions should also pass `--observation`.

### `cu click <ref|x y> --app <app> [options]`

Targets a ref, coordinates, `--ax-path`, or OCR `--text`. Supports right,
double-click, modifiers, OCR index/region, `--no-verify`, and `--no-snapshot`.
Ref clicks try native AX actions before PID-targeted CGEvent fallback.
Clicks targeting text inputs require the post-action focused element to match
the intended ref/path and return `verification_failed` otherwise.

### `cu set-value <ref> <text> --app <app> [--observation <id>]`

Set `AXValue` directly without focus, IME, or clipboard. An `--ax-path`
selector may replace the ref where supported. When `controlled_editor_risk` is
true, AXValue may not reach an Electron/React editor's application state; never
use the write alone as proof that Submit/Send is enabled.

### `cu perform <ref> <AXAction> --app <app> [--observation <id>]`

Invoke actions such as `AXPress`, `AXShowMenu`, `AXIncrement`,
`AXDecrement`, `AXConfirm`, `AXCancel`, `AXOpen`, or `AXRaise`. Read
`dispatched` and `effect_verified`; mutation actions whose AX state stays
unchanged return `verification_failed`.

### `cu type <text> --app <app> [--paste|--no-paste] [--no-snapshot]`

Inject Unicode by PID. Automatically uses PID-targeted clipboard paste for
known chat apps and focused inputs below `AXWebArea`; native controls keep
Unicode events, including CJK. Read `paste_reason`. Targeted typing requires a focused AX text input.
`effect_verified:true` means the focused value changed;
An unobservable targeted result fails with `unknown_outcome`. Inputs below
`AXWebArea` get a second stability snapshot and remain medium confidence:
stable text does not generically prove every controlled-editor side effect.

### `cu key <combo> --app <app> [--no-snapshot]`

Send modifiers and keys through PID-targeted CGEvents. Event dispatch is
reported separately from observable effect. Enter on a focused input returns
`verification_failed` when neither the input nor surrounding AX state changes.

### `cu scroll <direction> [amount] --x <x> --y <y> --app <app>`

Scroll at screen coordinates.

### `cu hover <x> <y> --app <app>`

Move the real pointer to expose hover UI. This is visually disruptive even
with PID targeting.

### `cu drag <x1> <y1> <x2> <y2> --app <app> [modifiers]`

Move the real pointer through an interpolated drag and guarantee mouse-up.

## Script And System

### `cu tell <app> <expression> [--timeout seconds]`

Run an escaped, app-scoped AppleScript expression. Automation permission is
per target app and requested here, not by app discovery.

### `cu defaults read <domain> [key]`

Read a macOS defaults domain or key.

### `cu defaults write <domain> <key> <value...>`

Write a defaults value. Treat as a mutation and verify the consuming app's
state.

### `cu window <action> [args] [--app <app>] [--window N]`

Actions: `list`, `move`, `resize`, `focus`, `minimize`, `unminimize`, `close`.
Focus selects the AX window, activates the exact PID through
`NSRunningApplication`, and verifies the same PID through NSWorkspace. It
returns `focus_failed` instead of silently accepting AXRaise-only success.

### `cu launch <name|bundle-id> [--no-wait] [--timeout seconds]`

Launch through Launch Services and normally wait for an AX-ready window.

### `cu warm <app>`

Pay the first AX-walk cost before a latency-sensitive workflow.

## Recover

### `cu status`

Return private Broker PID/protocol plus command, active, and uncertain counts
for this client.

### `cu commands [--limit N] [--status <state>]`

List this client's command records.

### `cu command <command-id>`

Inspect one command descriptor.

### `cu cancel <command-id>`

Request cancellation. Cancellation after mutation dispatch can become
`unknown_outcome`; observe current UI before any retry.

See [recovery.md](recovery.md) for states and stable errors.
