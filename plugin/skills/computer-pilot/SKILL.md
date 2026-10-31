---
name: computer-pilot
description: Read and operate macOS app UI through the cu CLI and the Agent's existing shell, including native controls, text input, AppleScript, screenshots and OCR. Prefer Browser Pilot for web-page DOM tasks. Not needed for ordinary file edits.
---

# Computer Pilot

Use this skill and the existing shell to call `cu`. The private Broker is not
an Agent API; do not look for an MCP server, SDK or alternative transport.

## Start The Task

Use the caller's verified binary and permissions directly. Otherwise put
`${XDG_DATA_HOME:-$HOME/.local/share}/computer-pilot/bin` first on PATH and run
`cu --version` once. Computer Pilot prints `cu <semver>`; macOS's unrelated
`/usr/bin/cu` prints `cu (Taylor UUCP)`. For a missing/outdated binary, use
[installation](references/permissions.md#installation) and
[compatibility.json](compatibility.json). Run `cu setup` only after installation
or a permission error.

Choose the cheapest suitable interface. For a known scriptable app's data,
use `cu tell`; consult [scripting.md](references/scripting.md) if needed.
Use `cu apps` once when the target or scripting support is unknown. For UI,
start with an AX snapshot; use screenshots/OCR when AX lacks the needed content.

For multiple rounds, save the task configuration once, for example
`/absolute/task/cu-env.sh`:

```bash
export CU_BIN="/verified/path/to/cu"
export CU_APP="pid:89806"
export COMPUTER_PILOT_CLIENT_KEY="<unique-stable-task-key>"
export COMPUTER_PILOT_OUTPUT_DIR="/absolute/task/output"
# Preserve any caller-supplied COMPUTER_PILOT_HOME here too.
. /absolute/skill-directory/scripts/session.sh
```

Resolve [scripts/session.sh](scripts/session.sh) relative to this SKILL.md.
Keep the same PID, client key and supplied environment throughout the task.
Source the task file in each shell call. Its `cu` function supplies the target;
do not repeat `--app` or a positional app. Start with `cu snapshot`.

## One Call Per Decision Boundary

From the current observation, group all known field changes and authorized
Save/Next actions **with their final snapshot or wait in the same shell call**.
For example, if refs 7, 12 and 15 are the observed fields and Save button:

```bash
. /absolute/task/cu-env.sh
cu set-value 7 "Quarterly review"
cu set-value 12 "Draft"
cu click 15
cu snapshot
```

Do not end one tool call after Save only to request its snapshot in the next.
Make the next model decision from the group result. Stop the group when a new
page, dialog or unknown control needs a decision. Grouping does not authorize
additional external effects.

- Use `set-value` for native textfields, textareas and editable comboboxes.
- Leave already-correct values, checkboxes and selections alone.
- For a dropdown that needs changing, open it and observe the menu. Select the
  observed item with `perform <ref> AXPress` (or its supported action).
  Writing the popup's AXValue may not select the item or fire its handler.
- Skip per-field and pre-save inspections. After a recoverable error, refresh
  the relevant state and resume grouping; do not turn the rest of the workflow
  into one-field-at-a-time checks.

The helper omits action snapshots/effect checks and stops on command errors.
The final observation checks the group's result. For uncertain input delivery,
especially controlled editors, use the verified direct-CLI path below instead.

## Observe Transitions

Use `snapshot` for the next decision, or `wait` for a known asynchronous
condition. Include either at the end of the action group. A wait already returns
a complete snapshot and usable refs; do not immediately snapshot it again.

`wait --text` matches substrings in **any element's label or value**. Choose
text that distinguishes the destination from the current UI. A word already in
"Next: …", a source label, or an unchanged field does not establish arrival.
Inspect an unexpected confirmation dialog before waiting for the state beyond
it; use `wait --modal` when the expected next state is a dialog.

AX actions can return before a menu closes or a view updates. If the observation
still shows the departing menu/view, observe again or wait for the distinguishing
next state; do not repeat the accepted action. Do not add fixed sleeps.

## Target And Ref Identity

Use an exact `pid:<PID>` or a unique app/bundle name. For `ambiguous_target`,
choose a selector from the reported candidates. Resolve a new PID after exit;
never select a duplicate by whichever copy is active.

Refs belong to one client's observation, PID, window and target signature.
Ordinary actions use that client's latest observation. `--observation <id>` is
only needed to choose an earlier observation. A ref number in a later snapshot
may identify another element. After `stale_observation`, observe once and
re-plan; do not retry the old ref or switch to OCR merely because it expired.
Use `find` for a missing known label/role, or `why` for an unclear control.
A window close button is not a navigation shortcut.

Outside the helper, pass `--app` to actions. Keep background work PID-targeted;
never respond to delivery failures by dropping the target, using global input,
or automatically focusing the app. `window focus` and `--allow-global` are
visible/disruptive and require a workflow that explicitly needs foreground
operation. `launch` opens in the background. Do not recreate input through
System Events or `osascript`/`pbcopy`.

## Controlled Editors And Recovery

For Electron/CEF or an `AXWebArea` input, read the
[verified-input path](references/recovery.md#verification). Use the verified
binary directly with an explicit app: require exact editor focus before typing
and correct final text after typing. A successful AXValue write alone does not
prove the application handled input. Stop unconfirmed delivery; use an explicitly
provided development CDP/Playwright endpoint when available.

Success receipts report dispatch; observations and explicit verification report
effects. Read returned errors/advisories and false verification results. Reuse
attached state for recovery. A missing/failed snapshot is not permission to
repeat a mutation. For timeout/unknown outcome, inspect current state before
another mutation; use a request ID when a consequential action needs idempotent
recovery, not for every local field edit.

Keep recovery within the authorized task. Do not change login-session
environment or restart an app to work around a missing control. Report blocked
when recovery needs setup or permissions outside that scope. See
[recovery.md](references/recovery.md) for command inspection and error codes.

## Output And Further Commands

The helper defaults to concise JSON and complete snapshots up to 200 elements,
including unchanged controls and full values. Increase `--limit` only when a
truncation hint says needed content is missing. `--compact`, `--json` and
`--human` before the command retain task binding; `--json` adds geometry/axPaths,
not a different state model. For other root options or default action
verification, call `"$CU_BIN"` directly with the explicit target.

For a single short task, direct CLI actions may use `--no-snapshot` and
`--no-verify`, with one final observation. Use `state` when window/display
metadata or an image is needed. File outputs need an absolute task-owned output
directory; read returned paths through the host's normal file capability.

Use `cu <command> --help` for unfamiliar syntax. Read only the relevant reference:

- [commands.md](references/commands.md): CLI options and command inventory.
- [visual.md](references/visual.md): screenshots, OCR, coordinates and file output.
- [permissions.md](references/permissions.md): installation and permission errors.
- [embedding.md](references/embedding.md): packaging the skill for an Agent host.
