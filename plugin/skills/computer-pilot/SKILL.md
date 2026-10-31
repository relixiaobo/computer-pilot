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

For repeated UI actions, keep a fixed task working directory. Copy the packaged
helper into `./cu-env.sh`, append the task values, and take the first UI observation
in one shell call. Resolve the helper path relative to this SKILL.md:

```bash
cp /absolute/skill-directory/scripts/session.sh ./cu-env.sh
cat >> ./cu-env.sh <<'EOF'
export CU_BIN="/verified/path/to/cu"
export CU_APP="pid:89806"
export COMPUTER_PILOT_CLIENT_KEY="<unique-stable-task-key>"
export COMPUTER_PILOT_OUTPUT_DIR="/absolute/task/output"
# Preserve any caller-supplied COMPUTER_PILOT_HOME here too.
EOF
. ./cu-env.sh
cu snapshot
```

The copied [helper](scripts/session.sh) contains the `cu` function and stops the
shell on errors; it can be used without reading its implementation. Source only
`./cu-env.sh` in each subsequent shell call; use its absolute path if cwd changes.
Keep the supplied PID, binary and environment in that file throughout the task.
Its `cu` function supplies the PID for UI commands (including `window`); omit
repeated `--app` or positional apps. It also supplies `--no-snapshot` for actions
and `--no-verify` for click/type/key. Other commands keep ordinary CLI arguments:
`cu sdef "App Name"` and `cu tell "App Name" '<expression>'` require an explicit
application name or bundle path; AppleScript does not accept `pid:<PID>`.

## One Call Per Decision Boundary

From the current observation, group all known field changes and authorized
Save/Next actions **with their final snapshot or wait in the same shell call**.
For example, if refs 7, 12 and 15 are the observed fields and Save button:

```bash
. ./cu-env.sh
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
  For Electron/CEF or fields inside `AXWebArea`, use the verified click/type
  path under Controlled Editors below; AXValue writes can bypass editor handlers.
- Leave already-correct values, checkboxes and selections alone.
- For a dropdown that needs changing, open it and observe the menu. Select the
  observed item with `perform <ref> AXPress` (or its supported action).
  Observe the resulting controls before further edits or navigation.
  Writing the popup's AXValue may not select the item or fire its handler.
- Skip per-field and pre-save inspections. After a recoverable error, refresh
  the relevant state and resume grouping; do not turn the rest of the workflow
  into one-field-at-a-time checks.

The helper omits action snapshots/effect checks and stops on command errors.
The final observation checks the group's result. For uncertain input delivery,
especially controlled editors, use the verified direct-CLI path below instead.

## Observe Transitions

Choose the group's final observation from what is already known:

- Unfamiliar navigation: `snapshot` to discover what opened, including dialogs.
- Known asynchronous transition: the action followed directly by `wait` in the
  same shell call. Once the flow has been observed, reuse it on subsequent items.
  For example, when an observed confirmation returns to a list headed "All records":

  ```bash
  . ./cu-env.sh
  cu click 15
  cu wait --text "All records"
  ```

The wait returns the destination snapshot and usable refs; that is the group's
observation. Omit a preliminary snapshot of the known loading state and a second
snapshot after the wait. Use the default 10-second wait timeout unless the task
needs another bound; `wait --timeout 20` means **20 seconds**.

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
may identify another element. After `stale_observation`, re-plan from the attached
`diagnostics.snapshot`; observe once only when none is attached. Do not retry the
old action or switch to OCR merely because its ref expired.
Use `find` for a missing known label/role, or `why` for an unclear control.
A window close button is not a navigation shortcut.

Outside the helper, pass `--app` to actions. Keep background work PID-targeted;
never respond to delivery failures by dropping the target, using global input,
or automatically focusing the app. `window focus` and `--allow-global` are
visible/disruptive and require a workflow that explicitly needs foreground
operation. `launch` opens in the background. Do not recreate input through
System Events or `osascript`/`pbcopy`.

AX editor focus does not make an app frontmost. Some apps receive background
shortcuts but ignore menu commands such as Select All or Save. Prefer app-native
scripting for document operations; when foreground editing is authorized, focus
the intended window explicitly. A changed editor buffer is not proof of saving:
use the app's saved/modified state at the final group boundary.

## Controlled Editors And Recovery

For Electron/CEF or an `AXWebArea` input, use verified click/type instead of
AXValue writes. Group the known edit in one shell call; the CLI stops on focus
or input failure. Omit the click when the intended editor is already focused:

```bash
. ./cu-env.sh
"$CU_BIN" --compact click <editor-ref> --app "$CU_APP" --limit 200 --no-snapshot
"$CU_BIN" --compact type "<intended text>" --app "$CU_APP"
```

The click keeps its private focus check; typing returns the group's observation.
Virtual editors can expose only a document slice or add boundary newlines in AX.
Resolve text discrepancies using the editor's document view and cursor/line state
before changing the content again; AX text is not a saved-file byte dump.
If AX lacks the text, inspect visible evidence before deciding input failed; do
not replay it to obtain a verification flag. See
[recovery.md](references/recovery.md#verification) for uncertain outcomes.

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
