---
name: computer-pilot
description: Control macOS desktop applications through the `cu` CLI and an Agent's existing shell. Use for reading or operating native app UI, clicking controls, filling fields, typing, keyboard shortcuts, window management, AppleScript-capable app data, screenshots, or OCR. Prefer Browser Pilot for web-page DOM tasks. Do not use for text-only file edits that do not require GUI interaction.
---

# Computer Pilot

Use only `Agent -> this skill -> existing shell -> cu CLI`. Never look for or
expose MCP, stdio, JSON-RPC, a native SDK, or Agent-specific adapters. The
per-user Broker is a private `cu` implementation detail.

## Initialize

If the caller has already verified the Computer Pilot binary and permissions,
start directly with the task. Otherwise run this preflight once per session.
Put the managed install directory first on `PATH`
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

1. If the target or its scripting support is unknown, use `cu apps` once.
   For a known app, go directly to the relevant command.
2. Use `cu tell` for data-level work in scriptable apps.
3. Use AX observation and actions for ordinary UI work.
4. Use OCR or screenshots only when AX is sparse or visual state matters.

For Browser UI, use Browser Pilot when available. Read
[scripting.md](references/scripting.md) for AppleScript and
[visual.md](references/visual.md) for screenshot/OCR workflows.

## Core State-Act-Verify Loop

For ordinary UI work, start with one AX snapshot of the known app:

```bash
cu snapshot "Mail" --limit 200
```

Use `cu state` only when window/display metadata or a screenshot is needed.
Reuse the returned PID selector for actions. Prefer `set-value` for native
fields that support it, avoiding click/select/type entirely.

Batch deterministic actions in one shell call. Stop the shell on errors and
read the final result before deciding the next group:

```bash
set -e
cu key cmd+a --app "pid:89806" --no-snapshot --no-verify
cu type "Quarterly review" --app "pid:89806"
```

This example requires that the intended editor is already focused. When
focus is unknown, click the observed field with default verification first;
the shell can then continue to typing if that command succeeds. Do not add a
model round just to repeat a check the command already performed.

For intermediate `click`, `key`, and `type`, `--no-verify --no-snapshot`
defers effect observation to the group boundary. `set-value`, `perform`,
`scroll`, `hover`, and `drag` need only `--no-snapshot`. Keep the final action's
snapshot, or finish with `cu snapshot`. These opt-outs acknowledge dispatch,
not task success; verify the expected final value/state before continuing.

For several independent native fields already visible in the observation,
write them together and inspect the final snapshot once:

```bash
set -e
cu set-value 7 "Quarterly review" --app "pid:89806" --observation "<id>" --no-snapshot
cu set-value 12 "Draft" --app "pid:89806" --observation "<id>"
```

The same observation remains valid for the second field when only the first
field's content changed. Once the final snapshot contains the requested values,
stop; do not add another observation to reconfirm those same values.

Only group actions whose targets and dependencies are already known. Stop
and observe after navigation, a new dialog, unknown focus, or any action whose
result determines the next target. Do not reuse pre-group refs across layout
changes. Never include a submit/send/delete after unverified input merely to
save a round trip. No fixed sleeps or repeated unchanged observations: use a
condition wait when a specific asynchronous result is expected.

Use waits for asynchronous transitions:

```bash
cu wait --text "Saved" --app "Mail" --timeout 10
cu wait --new-window --app "Mail" --timeout 5
cu wait --modal --app "Mail" --timeout 5
```

## Targeting Rules

Pass `--app` to `click`, `type`, `key`, `scroll`, `hover`, `drag`, `set-value`,
and `perform` when targeting a background app. Without `--app`, input is
delivered to the current frontmost PID without moving the user's cursor. Treat
any `*-global` method as disruptive: it appears only when `--allow-global` was
explicitly supplied. Terminal and IDE frontmost targets still require an
explicit `--app`; do not answer that refusal with `--allow-global`.
Treat `cu window focus` as explicitly disruptive and user-visible; never use it
as an automatic fallback. `cu launch` opens apps in the background and never
takes the user's focus. `cu tell` refuses `activate` and System Events
`keystroke`/`key code` — use PID-targeted `cu key`/`cu type` instead.

The private Broker can optionally show a click-through software cursor for the
human observer. Set `COMPUTER_PILOT_VISUAL_CURSOR=1` before the first `cu`
command in a session. It applies to pointer actions only, follows the target
window without moving the real cursor, and is never part of agent screenshots.
The overlay is presentation-only and may fail open without changing the action.

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

1. Use a ref already present in the current Observation.
2. Use `cu find` when the target is missing and its role or label is known.
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
3. Type the intended text once with default verification. Continue only when
   the final text is correct. `cu type` automatically uses PID-targeted paste for
   a focused input below `AXWebArea`; do not override it with `--no-paste`.
4. Stop and inspect the current state on
   `verification_failed`, `effect_verified:false`, or missing focus.

For a dev Electron app with an explicitly provided localhost CDP/Playwright
endpoint, switch to it. Otherwise report unconfirmed delivery. Never drop
`--app`, use global System Events keystrokes, or activate by app name.

If an element path contains `webarea`, do not use `cu set-value` as proof that
a React/ProseMirror-style controlled editor handled input. AXValue can change
without firing its input/onChange handler or enabling Send.

## Observation And Ref Safety

Refs are ephemeral and bind to one Observation's client, PID, bundle, window,
and target element signature. Unrelated elements may change without invalidating
the target. A changed target, PID, or window still fails before dispatch.

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

`--no-snapshot` only suppresses output; use `--no-verify` as well on
intermediate click/type/key actions to defer effect verification. A generic
shortcut or AX action has no universal success predicate: `dispatched:true`
requires checking its intended result at the group boundary.

`cu type` may route through clipboard paste for a focused `AXWebArea` input or
a known chat app. Native inputs keep Unicode events, including CJK. Read
`paste_reason`; do not recreate typing with `osascript` or `pbcopy`. A targeted
type requires a focused AX text input and fails before dispatch otherwise.

Action snapshots omit `axPath` and cap long text, but retain the requested
element limit. When `snapshot.base_observation_id` is present, `elements`
contains only new or changed refs: apply them to that prior observation and
remove `snapshot.removed_refs`. Unchanged refs retain their previous content;
all refs now belong to the response's new `observation_id`. Focus/window
metadata is always current. If the baseline is unavailable, request a full
`cu snapshot`. Do not issue a new snapshot just because a delta is empty.

Always react to top-level fields ending in `_hint`, `_reason`, `_advice`, or
`_error`. They mark degraded, partial, or automatically corrected output.

## File Results

Set `COMPUTER_PILOT_OUTPUT_DIR` to an absolute task-owned directory. File
commands return the legacy path plus structured metadata containing absolute
`path`, `mime`, `bytes`, `width`, `height`, and `scale`.

Writes are atomic `0600`, do not overwrite, and reject relative paths or
symlink traversal. Read returned paths with the host's normal file capability.

## Recovery

Use `--request-id` when a consequential or long-running mutation needs
idempotent recovery after a timeout. Ordinary local field edits do not need it:

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
