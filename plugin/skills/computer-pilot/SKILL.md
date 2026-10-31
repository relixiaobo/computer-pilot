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
Prefer `set-value` for native fields; it avoids click/select/type.

Group known actions in one shell call, including filling fields and an
already-authorized Save/Next. Observe once at the boundary where a new page,
dialog or unknown target requires another decision. Do not add a pre-save
inspection, per-field check, or second confirmation of an observed result.
Use `cu wait` for a specific asynchronous condition, never a fixed sleep.
External effects still require the user's authorization; grouping adds none.

For a task with multiple rounds, save its setup **once** in a task-owned shell
file (use the verified binary and exact PID returned by the first observation):

```bash
# /absolute/task/cu-env.sh
export CU_BIN="/verified/path/to/cu"
export CU_APP="pid:89806"
export COMPUTER_PILOT_CLIENT_KEY="<stable-task-key>"
# Set COMPUTER_PILOT_OUTPUT_DIR here only if creating screenshots/files.
. /absolute/path/to/computer-pilot/scripts/session.sh
```

Reuse that file in each shell invocation instead of rewriting environment,
PID, observation IDs and action flags. The bundled helper is ordinary shell;
`cu_do` invokes the CLI with this PID, skips intermediate snapshots/effect
checks, and stops the group on errors. `cu_see` returns one compact observation.
For example, when refs 7, 12 and Save button 15 are in the current observation:

```bash
. /absolute/task/cu-env.sh
cu_do set-value 7 "Quarterly review"
cu_do set-value 12 "Draft"
cu_do click 15
cu_see
```

The latest observation for this task and PID is selected automatically; repeat
`--observation` only when deliberately binding an older observation. Ref and
PID freshness checks remain active. Do not reuse pre-navigation refs after
layout changes. Intermediate dispatch is not proof of the final task outcome;
read the boundary state and stop once the requested outcome is shown.

For a short task, use the CLI directly. Intermediate actions take
`--no-snapshot`; `click`, `type` and `key` also take `--no-verify`. Keep the
final action's attached snapshot or issue one explicit snapshot in the same
shell call. Do not add a model round solely to inspect an intermediate result.

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
axPaths, routine command metadata and echoed `set-value` arguments, while
retaining labels, values, target PID, observation IDs, effect/focus results,
and all advisories/errors. Use ordinary `--json` when coordinates, full
metadata or machine integration are needed. Error and recovery output stays
complete in both modes.

Action snapshots and compact explicit snapshots return changed refs when a
baseline exists. With `base_observation_id`, merge `elements` into that prior
observation and remove `removed_refs`; unchanged refs retain their content.
The new `observation_id` applies to the whole resulting tree. An empty delta
is valid. If the baseline is unavailable, use `cu --json snapshot <PID>` once
for a full tree. Explicit snapshots retain full text; action snapshots may cap
long values and include `snapshot_hint` when they do.

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
