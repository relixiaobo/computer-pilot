---
name: computer-pilot
description: Control macOS desktop applications through the `cu` CLI and an Agent's existing shell. Use for reading or operating native app UI, clicking controls, filling fields, typing, keyboard shortcuts, window management, AppleScript-capable app data, screenshots, or OCR. Prefer Browser Pilot for web-page DOM tasks. Do not use for text-only file edits that do not require GUI interaction.
---

# Computer Pilot

Use only `Agent -> this skill -> existing shell -> cu CLI`. Never look for or
expose MCP, stdio, JSON-RPC, a native SDK, or Agent-specific adapters. The
per-user Broker is a private `cu` implementation detail.

## Initialize

Run this preflight before the first desktop action. Do not assume the host
installed Computer Pilot. Put the managed install directory first on `PATH`
so every later `cu` in this skill resolves to Computer Pilot:

```bash
export PATH="${XDG_DATA_HOME:-$HOME/.local/share}/computer-pilot/bin:$PATH"
cu --version
```

macOS ships an unrelated `/usr/bin/cu` (the UUCP serial dialer) that answers
`cu --version` with `cu (Taylor UUCP) 1.07`. Computer Pilot always prints
`cu <semver>`. Treat any other output — and any failure — as "not
installed"; never run desktop commands against the UUCP binary.

Install when the preflight does not print `cu <semver>`, or when that version
is below `cli.minimum_version` in
[compatibility.json](compatibility.json). Read the manifest's `installation`
object and invoke its `installer` with `sh`, resolving the path inside this
skill directory — always `sh <installer>`, never `./<installer>`, because a
packaged copy may arrive without the executable bit:

```bash
sh scripts/install-native.sh \
  --version "<manifest version>" \
  --repository "<installation.repository>" \
  --asset-template "<installation.asset_template>" \
  --allow-unsigned
```

Replace `--allow-unsigned` with `--requirement "<installation.signing.requirement>"`
when that value is set; pass `--allow-unsigned` only when the manifest
declares `required_status` `ad-hoc-unsigned`. Never install with sudo, a
`latest` URL, or any download outside this installer. Re-run the two
preflight lines afterwards; the installer also reports the absolute
`command` path if you need it.

Establish stable task identity:

```bash
export COMPUTER_PILOT_CLIENT_KEY="<stable-logical-agent-key>"
export COMPUTER_PILOT_OUTPUT_DIR="/absolute/task-owned/output-dir"
```

Keep the client key stable for one logical Agent and use another key for each
other Agent. Set an absolute output directory before creating screenshots.

Run `cu setup` only on first use or after a permission error. It checks core
Accessibility and Screen Recording access. Automation is granted separately
for each target app and is requested only by `cu tell`.

## Choose A Control Tier

Use the cheapest reliable tier:

1. Use `cu apps` to discover running apps and the `S` scripting flag.
2. Use `cu tell` for data-level work in scriptable apps.
3. Use AX observation and actions for ordinary UI work.
4. Use OCR or screenshots only when AX is sparse or visual state matters.

For Browser UI, use Browser Pilot when available. Read
[scripting.md](references/scripting.md) for AppleScript and
[visual.md](references/visual.md) for screenshot/OCR workflows.

## Core State-Act-Verify Loop

Start a desktop task with one state call:

```bash
cu state "Mail"
```

It returns AX elements, windows, displays, frontmost state, an
`observation_id`, and normally a screenshot. Use `--no-screenshot` when AX is
sufficient.

Inspect the returned state, then perform exactly one action with the same
explicit app selector:

```bash
cu click 12 --app "Mail" --observation "<observation_id>"
cu set-value 7 "Quarterly review" --app "Mail" --observation "<observation_id>"
cu key cmd+enter --app "Mail"
cu type "Hello" --app "Mail"
```

Read the attached `snapshot` before the next action. Do not shell-chain UI
mutations; state can change between them.

Use waits for asynchronous transitions:

```bash
cu wait --text "Saved" --app "Mail" --timeout 10
cu wait --new-window --app "Mail" --timeout 5
cu wait --modal --app "Mail" --timeout 5
```

## Targeting Rules

Always pass `--app` to `click`, `type`, `key`, `scroll`, `hover`, `drag`,
`set-value`, and `perform`. PID-targeted delivery avoids focus drift and keeps
the user's frontmost app unchanged. Treat any `*-global` method as disruptive.
Without `--app`, global-tap actions (type, key, coordinate/`--text` click,
scroll, hover, drag) are refused when the user's frontmost app is a terminal
or IDE; do not answer that refusal with `--allow-global` — pass `--app`.
Treat `cu window focus` as explicitly disruptive and user-visible; never use it
as an automatic fallback. `cu launch` opens apps in the background and never
takes the user's focus. `cu tell` refuses `activate` and System Events
`keystroke`/`key code` — use PID-targeted `cu key`/`cu type` instead.

Use a unique application name, bundle identifier, or `pid:<PID>`. `cu apps`
returns each process's `selector`. Ambiguous names/bundles fail without acting;
choose from `diagnostics.candidates` and reuse that PID for the whole loop:

```bash
cu state "pid:89806"
cu click 12 --app "pid:89806" --observation "<observation_id>"
cu wait --text "Saved" --app "pid:89806" --timeout 10
```

Never select a duplicate by whichever copy is active. After restart, resolve a
new PID and create a fresh Observation.

Prefer targeting in this order:

1. Use `cu find` when role or label is known.
2. Use a ref from the current Observation.
3. Use `axPath` for a known structural selector.
4. Use `cu nearest` or `cu observe-region` for visual coordinates.
5. Use `cu click --text` when only OCR can see the label.
6. Use raw coordinate clicks only as the final fallback.

```bash
cu find --app "Mail" --role button --title-equals "Send" --first
cu click 12 --app "Mail" --observation "<observation_id>"
```

Do not pipe a newly discovered ref directly into an action without retaining
the observation that produced it.

## Electron And Controlled Editors

Use this bounded path for Electron/CEF editors:

1. Select the exact `pid:<PID>` and retain it across the loop.
2. Click the editor and require `focus_verified:true` before typing.
3. Type a short prefix of the intended text. Continue only when
   `effect_verified:true`. `cu type` automatically uses PID-targeted paste for
   a focused input below `AXWebArea`; do not override it with `--no-paste`.
4. After one focus attempt and one prefix probe, stop UI retries on
   `verification_failed`, `effect_verified:false`, or missing focus.

For a dev Electron app with an explicitly provided localhost CDP/Playwright
endpoint, switch to it. Otherwise report unconfirmed delivery. Never drop
`--app`, use global System Events keystrokes, or activate by app name.

If an element path contains `webarea`, do not use `cu set-value` as proof that
a React/ProseMirror-style controlled editor handled input. AXValue can change
without firing its input/onChange handler or enabling Send.

## Observation And Ref Safety

Refs are ephemeral and bind to one Observation's client, PID, bundle, window,
AX generation, and element signatures.

- Pass `--observation <id>` for explicit ref actions.
- Without it, `cu` resolves the latest Observation for this client and target.
- Never reuse another Agent's Observation or ref.
- Never treat the same integer in a later snapshot as the same element.
- On `stale_observation`, do not retry the action. Observe again and re-plan.
- User activity can invalidate an Observation; Computer Pilot never locks out
  the user to preserve stale state.

An `axPath` avoids ref renumbering but does not make intent permanent. Verify
the post-action snapshot.

## Action Results

Actions return `method`, `dispatched`, and sometimes `effect_verified`.
`dispatched:true` proves only macOS accepted the request. Continue only after
the expected state is visible or `effect_verified:true`; unobservable targeted
typing returns `unknown_outcome`.

`cu click` verifies AX change by default. Read `verified`, `verify_diff`, and
`verify_advice`. `verified:true` means the tree changed, not that the intended
business outcome occurred. Confirm the attached snapshot. Use `--no-verify`
only for a known reliable bulk workflow.

`cu type` may route through clipboard paste for a focused `AXWebArea` input or
a known chat app. Native inputs keep Unicode events, including CJK. Read
`paste_reason`; do not recreate typing with `osascript` or `pbcopy`. A targeted
type requires a focused AX text input and fails before dispatch otherwise.

Action snapshots omit `axPath`, cap text, and include at most 50 elements. Run
`cu snapshot` when the next target is absent.

Always react to top-level fields ending in `_hint`, `_reason`, `_advice`, or
`_error`. They mark degraded, partial, or automatically corrected output.

## File Results

Set `COMPUTER_PILOT_OUTPUT_DIR` to an absolute task-owned directory. File
commands return the legacy path plus structured metadata containing absolute
`path`, `mime`, `bytes`, `width`, `height`, and `scale`.

Writes are atomic `0600`, do not overwrite, and reject relative paths or
symlink traversal. Read returned paths with the host's normal file capability.

## Recovery

Assign `--request-id` before a mutation that may need idempotent recovery:

```bash
cu --request-id "send-draft-17" click 12 --app "Mail" --observation "<id>"
cu status
cu commands --limit 20
cu command "<command_id>"
cu cancel "<command_id>"
```

Use root `--timeout <milliseconds>`. A post-dispatch timeout returns
`unknown_outcome`; inspect UI state before another mutation.

Branch on stable `code`, not English error text. Never blindly retry a
mutation. Read [recovery.md](references/recovery.md) for the error matrix,
command states, verification recovery, and routing methods.

## Command Discovery

Use runtime help as the authoritative inventory:

```bash
cu --help
cu <command> --help
```

Read references only when needed:

- [commands.md](references/commands.md): command groups, important flags, and output fields.
- [scripting.md](references/scripting.md): `cu apps`, `cu sdef`, and `cu tell`.
- [visual.md](references/visual.md): screenshot, annotation, OCR, and coordinate mapping.
- [recovery.md](references/recovery.md): stable errors, request IDs, cancellation, and method routing.
- [permissions.md](references/permissions.md): Accessibility, Screen Recording, and per-app Automation.
- [embedding.md](references/embedding.md): install and bundle Computer Pilot in any shell-capable Agent.

For unknown or newly added flags, trust `cu <command> --help` over remembered
syntax.
