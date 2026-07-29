---
name: computer-pilot
description: Control macOS desktop applications through the `cu` CLI and an Agent's existing shell. Use for reading or operating native app UI, clicking controls, filling fields, typing, keyboard shortcuts, window management, AppleScript-capable app data, screenshots, or OCR. Prefer Browser Pilot for web-page DOM tasks. Do not use for text-only file edits that do not require GUI interaction.
---

# Computer Pilot

Use the public integration path only:

```text
Agent -> this skill -> existing shell -> cu CLI
```

Never look for or expose a Computer Pilot MCP server, stdio bridge, JSON-RPC
tool catalog, native SDK, or Agent-specific adapter. The per-user Broker is a
private `cu` implementation detail.

## Initialize

Confirm the binary and establish stable task identity:

```bash
cu --version
export COMPUTER_PILOT_CLIENT_KEY="<stable-logical-agent-key>"
export COMPUTER_PILOT_OUTPUT_DIR="/absolute/task-owned/output-dir"
```

Keep the client key stable across commands and recovery attempts for one
logical Agent. Use a different key for another Agent. Set an absolute output
directory before commands that create screenshots.

Run `cu setup` only on first use or after a permission error. It checks core
Accessibility and Screen Recording access. Automation is granted separately
for each target app and is requested only by `cu tell`.

Read [permissions.md](references/permissions.md) when setup or TCC access fails.

## Choose A Control Tier

Use the cheapest reliable tier:

1. Use `cu apps` to discover running apps and the `S` scripting flag.
2. Use `cu tell` for data-level work in scriptable apps.
3. Use AX observation and actions for ordinary UI work.
4. Use OCR or screenshots only when AX is sparse or visual state matters.

For Browser UI, use Browser Pilot when available; DOM semantics are more
precise than macOS AX for page content.

Read [scripting.md](references/scripting.md) for AppleScript workflows. Read
[visual.md](references/visual.md) for screenshots, OCR, and VLM workflows.

## Core State-Act-Verify Loop

Start a desktop task with one state call:

```bash
cu state "Mail"
```

It returns the AX elements, windows, displays, frontmost state, an
`observation_id`, and normally a screenshot. Use `--no-screenshot` when the AX
tree is sufficient.

Inspect the returned state, then perform exactly one action with the same
explicit app selector:

```bash
cu click 12 --app "Mail" --observation "<observation_id>"
cu set-value 7 "Quarterly review" --app "Mail" --observation "<observation_id>"
cu key cmd+enter --app "Mail"
cu type "Hello" --app "Mail"
```

Read the action's auto-attached `snapshot` before choosing the next action.
Avoid shell chains of multiple UI mutations; state can change between them.

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

An app selector may be a unique application name, a unique bundle identifier,
or `pid:<PID>`. `cu apps` returns the exact `selector` and `bundle_path` for
each running process. If a name or bundle identifier matches multiple
instances, `cu` returns `ambiguous_target` and does not observe or act. Choose
the intended process from `diagnostics.candidates`, then reuse its PID selector
for the entire state-act-verify loop:

```bash
cu state "pid:89806"
cu click 12 --app "pid:89806" --observation "<observation_id>"
cu wait --text "Saved" --app "pid:89806" --timeout 10
```

Never select a duplicate instance by whichever copy is active. A PID expires
when the process exits; after restart, run `cu apps`, obtain the new selector,
and create a fresh Observation.

Prefer targeting in this order:

1. Use `cu find` when role or label is known.
2. Use a ref from the current Observation.
3. Use `axPath` for a known structural selector.
4. Use `cu nearest` or `cu observe-region` for visual coordinates.
5. Use `cu click --text` when only OCR can see the label.
6. Use raw coordinate clicks only as the final fallback.

Example:

```bash
cu find --app "Mail" --role button --title-equals "Send" --first
cu click 12 --app "Mail" --observation "<observation_id>"
```

Do not pipe a newly discovered ref directly into an action without retaining
the observation that produced it.

## Observation And Ref Safety

Refs are ephemeral and belong to one Observation. An Observation binds them to
the client key, PID, bundle identifier, AX window identifier, AX generation,
and element signatures.

- Pass `--observation <id>` for explicit ref actions.
- Without it, `cu` resolves the latest Observation for this client and target.
- Never reuse another Agent's Observation or ref.
- Never treat the same integer in a later snapshot as the same element.
- On `stale_observation`, do not retry the action. Observe again and re-plan.
- User activity can invalidate an Observation; Computer Pilot never locks out
  the user to preserve stale state.

An `axPath` avoids ref renumbering but does not make UI intent permanent. Read
the post-action snapshot and verify the expected outcome.

## Action Results

Actions return a `method` field. Prefer `ax-action`, `ax-set-value`, and
`ax-perform`; `*-pid` is targeted; `*-global` is disruptive.

`cu click` verifies AX change by default. Read `verified`, `verify_diff`, and
`verify_advice`. `verified:true` means the tree changed, not that the intended
business outcome occurred. Confirm the attached snapshot. Use `--no-verify`
only for a known reliable bulk workflow.

`cu type` may route through clipboard paste for CJK or chat apps. Read
`paste_reason`; do not recreate typing with `osascript` or `pbcopy`.

Always react to top-level fields ending in `_hint`, `_reason`, `_advice`, or
`_error`. They mark degraded, partial, or automatically corrected output.

## File Results

Set `COMPUTER_PILOT_OUTPUT_DIR` to an absolute task-owned directory. File
commands return the legacy path plus structured metadata containing absolute
`path`, `mime`, `bytes`, `width`, `height`, and `scale`.

Computer Pilot writes atomically with mode `0600`, refuses overwrite, and
rejects relative paths and symlink traversal. Read the returned local path with
the Agent host's normal image/file capability; no MCP image result is needed.

## Recovery

Assign `--request-id` before a mutation that may need idempotent recovery:

```bash
cu --request-id "send-draft-17" click 12 --app "Mail" --observation "<id>"
cu status
cu commands --limit 20
cu command "<command_id>"
cu cancel "<command_id>"
```

Use root `--timeout <milliseconds>` for the Broker deadline. A timeout after a
mutation dispatch returns `unknown_outcome`; inspect current UI state before
deciding whether another mutation is safe.

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
