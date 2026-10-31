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

If the verified binary is missing or below `cli.minimum_version` in
[compatibility.json](compatibility.json), follow the pinned installation steps
in [permissions.md](references/permissions.md#installation). Run `cu setup`
only on first use or after a permission error, not before every task.

Establish stable task identity:

```bash
export COMPUTER_PILOT_CLIENT_KEY="<stable-logical-agent-key>"
export COMPUTER_PILOT_OUTPUT_DIR="/absolute/task-owned/output-dir"
```

Keep the client key stable for one logical Agent and use another key for each
other Agent. Set an absolute output directory before creating screenshots.

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

## Observe And Act In Groups

Use `cu --compact snapshot "<known app>" --limit 200` to start ordinary UI
work. Use `state` only when window/display metadata or a screenshot is needed.
Prefer `set-value` for native textfields/textareas and editable comboboxes.
For a dropdown, open it, observe its menu items, then use `perform <item> AXPress`
(or the supported action reported on failure). An AXValue write to a popup can
be accepted without selecting the option or firing its application handler.

Group known actions in one shell call, including filling fields and an
already-authorized Save/Next. Observe once when a new page, dialog or unknown
target requires another decision. Skip per-field/pre-save inspections and
repeated confirmation of an observed result. External effects still require
the user's authorization; grouping adds none.

For multiple rounds, save setup once in a task-owned shell file. The helper is
[scripts/session.sh](scripts/session.sh), **relative to this SKILL.md, not the
repository root**. Use its resolved absolute path:

```bash
# /absolute/task/cu-env.sh
export CU_BIN="/verified/path/to/cu"
export CU_APP="pid:89806"
export COMPUTER_PILOT_CLIENT_KEY="<stable-task-key>"
# Keep any supplied COMPUTER_PILOT_HOME and COMPUTER_PILOT_OUTPUT_DIR here too.
. /absolute/skill-directory/scripts/session.sh
```

Use the directory from which you read this skill. In this repository the helper
is `plugin/skills/computer-pilot/scripts/session.sh`.

Source that task file in each shell call. Its `cu` function wraps the verified
binary, retaining the task environment for reads, waits and actions. It supplies
the chosen app for UI commands; do not repeat `--app` or a positional app.
Actions omit intermediate snapshots/effect checks and stop the group on errors.
Use this grouping for known controls whose combined result can be inspected at
the boundary. For uncertain input delivery, use the ordinary CLI with default
verification as described below.
`cu snapshot` returns the complete concise state up to 200 elements, including
unchanged controls and full values; `cu wait` returns usable fresh refs.
For example, with refs 7, 12 and Save 15 in the current observation:

```bash
. /absolute/task/cu-env.sh
cu set-value 7 "Quarterly review"
cu set-value 12 "Draft"
cu click 15
cu wait --text "Saved"
```

Use `cu snapshot` at a synchronous boundary, or `cu wait` for a known asynchronous
condition; the wait's snapshot is already the observation, so do not snapshot
again. Do not add fixed sleeps. After `stale_observation`, observe once and use
the current target; do not cycle through old refs, coordinates and OCR. Use OCR
when the control lacks AX support, not when a ref merely expired.

The helper passes root-option invocations through unchanged. Use
`cu --json snapshot "$CU_APP" --limit 200` only when geometry or axPaths are needed;
`cu snapshot` already returns the full state. Other commands
such as `tell`, `sdef`, `launch` and recovery keep their ordinary CLI arguments.
For a single short task, direct CLI actions can use `--no-snapshot` and
`--no-verify`; inspect the result at the group boundary.

## Targeting Rules

Outside the session helper, pass `--app` to `click`, `type`, `key`, `scroll`,
`hover`, `drag`, `set-value` and `perform` when targeting a background app. Without `--app`, input is
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

When the session helper is loaded, start these calls with a root option to keep
default verification; supply the app explicitly:

```bash
cu --compact click <editor-ref> --app "$CU_APP"
# After reading the click's focus result:
cu --compact type "<intended text>" --app "$CU_APP"
```

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

- Ordinary ref actions use the latest observation for this client and target.
- Use `--observation <id>` only to select a particular earlier observation.
- Never reuse another Agent's Observation or ref.
- Never treat the same integer in a later snapshot as the same element.
- On `stale_observation`, do not retry the action. Observe again and re-plan.
- User activity can invalidate an Observation; Computer Pilot never locks out
  the user to preserve stale state.

An `axPath` avoids ref renumbering but does not make intent permanent.

## Output And Errors

`--compact` is concise JSON for ref-based agent work. It omits geometry,
axPaths, routine command metadata, echoed action arguments and routine
dispatch reminders. It retains observed labels/values, PID/observation identity,
effect/focus results and actionable advisories/errors. Success receipts report
dispatch; only observations and explicit verification report UI effects. Use ordinary `--json` when coordinates, full
metadata or machine integration are needed. Error and recovery output stays
complete in both modes.

All snapshots, including action and wait output, contain the complete tree
within the requested limit and full text. Use the returned `observation_id`
and refs directly; no baseline or merge is needed. If `truncation_hint` says
controls are missing, increase `--limit` before assuming a target is absent.

Act on `_hint`, `_reason`, `_advice`, `_error`, `verified:false`, and
`effect_verified:false`. A `snapshot_error` is missing state, not a reason to
repeat a mutation blindly. Default click/type verification remains available
for uncertain delivery (especially controlled editors). `verified:true` only
means AX changed; assess the requested outcome at the group boundary.

`cu type` uses PID-targeted clipboard paste for focused `AXWebArea` inputs or
known chat apps, and Unicode events for native inputs. It requires a focused
AX text input. Do not recreate this with `osascript` or `pbcopy`.

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
