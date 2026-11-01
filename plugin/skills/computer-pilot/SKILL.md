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

For repeated actions, create one executable launcher in a fixed task directory.
Put the supplied binary, target and environment in it; resolve the helper path
relative to this SKILL.md. Use this launcher for all commands, including help,
`sdef` and `tell`, in subsequent shell calls. No shell state needs to persist:

```bash
cat > ./pilot <<'EOF'
#!/bin/sh
export CU_BIN="/verified/path/to/cu"
export CU_APP="pid:89806"
export COMPUTER_PILOT_CLIENT_KEY="<unique-stable-task-key>"
export COMPUTER_PILOT_OUTPUT_DIR="/absolute/task/output"
# Preserve any caller-supplied COMPUTER_PILOT_HOME here too.
exec /bin/sh /absolute/skill-directory/scripts/session.sh "$@"
EOF
chmod 700 ./pilot
./pilot snapshot
```

The [helper](scripts/session.sh) supplies compact output and the fixed PID for UI
commands (including `window`); omit repeated `--app` or positional app selectors.
It preserves CLI verification and automatic snapshots. For known intermediate
actions, `--no-snapshot` omits their output tree; only `click`, `type` and `key`
accept `--no-verify`. Check the result at the group boundary. `tell` and `window`
accept neither flag. Other commands keep ordinary arguments:
`./pilot sdef "App Name"` accepts an app name or bundle identifier.
`./pilot tell "App Name" '<expression>' accepts an app name or bundle path;
AppleScript does not accept `pid:<PID>`.
Use the launcher's absolute path if cwd changes; do not fall back to bare `cu`.

## One Call Per Decision Boundary

Use `set -e` for a shell group so a failed command stops later mutations. From
the current observation, group known edits and authorized Save/Next actions
**with their final observation in the same shell call**.
For example, if refs 7, 12 and 15 are the observed fields and Save button:

```bash
set -e
./pilot set-value 7 "Quarterly review" --no-snapshot
./pilot set-value 12 "Draft" --no-snapshot
./pilot click 15
```

Use the last action's snapshot as the group result. Stop when a new page,
dialog or unknown control needs a decision. Grouping does not authorize
additional external effects.

- Use `set-value` for native textfields, textareas and editable comboboxes.
  For Electron/CEF or fields inside `AXWebArea`, use the verified click/type
  path under Controlled Editors below; AXValue writes can bypass editor handlers.
- Leave already-correct values, checkboxes and selections alone.
- For a dropdown that needs changing, open it and observe the menu. Select the
  observed item with `perform <ref> AXPress` (or its supported action).
  Observe the resulting controls before further edits or navigation.
  Writing the popup's AXValue may not select the item or fire its handler.
- After an error, use attached state to recover and resume grouping. Obtain a
  fresh observation only when the needed state is missing. Keep established
  local edits together; do not add a check round for every field.

## Observe Transitions

Choose the group's final observation from what is already known:

- Unfamiliar navigation: `snapshot` to discover what opened, including dialogs.
- Known asynchronous transition: the action followed directly by `wait` in the
  same shell call. Once the flow has been observed, reuse it on subsequent items.
  For example, when an observed confirmation returns to a list headed "All records":

  ```bash
  set -e
  ./pilot click 15 --no-snapshot --no-verify
  ./pilot wait --text "All records"
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

For one-off direct CLI calls, pass `--app` to actions. Keep background work PID-targeted;
never respond to delivery failures by dropping the target, using global input,
or automatically focusing the app. `window focus` and `--allow-global` are
visible/disruptive and require a workflow that explicitly needs foreground
operation. `launch` opens in the background. Do not recreate input through
System Events or `osascript`/`pbcopy`.

## Controlled Editors And Recovery

Use literal Find/Replace within the requested scope or a known selected range.
With repeated text, distinguish the intended occurrence/section before replacing.
Preserve unrelated content and final newlines by leaving them untouched. Avoid
reconstructing a whole document from AX text for local changes. Home/End can use
smart indentation or viewport semantics; key names and counted arrow presses do
not establish a selection across wrapped or indented text.

AX editor focus does not make an app frontmost. Some apps ignore background menu
shortcuts such as Select All or Save. Prefer native scripting for document data;
when the workflow permits and needs foreground editing, explicitly focus the
intended window before the edit group.

For Electron/CEF or an `AXWebArea` input, use verified click/type instead of
AXValue writes. Group the known edit in one shell call; the CLI stops on focus
or input failure. Omit the click when the intended editor is already focused:

```bash
set -e
./pilot click <editor-ref> --no-snapshot
./pilot type "<intended text>"
```

The click keeps its private focus check; typing returns the group's observation.
`value_changed:true` reports an AX value change, not exact input or saving;
`effect_verified` remains null. A click's `verify_diff` likewise reports tree
changes; only `focus_verified` establishes the scoped input-focus result.
Virtual editors can expose a slice or synthetic boundary newlines in AX.
Treat AX text as an observation, not saved-file bytes: do not add/delete boundary
characters or rewrite correct content merely to match that representation.
If the intended editor is unfocused and its AX value is empty, focus the observed
editor before concluding it lacks content. For an actual discrepancy, inspect
visible document/selection evidence or app-native data before another edit.
Read-only observation is the recovery for uncertain evidence, not another mutation.
See [recovery.md](references/recovery.md#verification) for uncertain outcomes.

Success receipts report dispatch. Read returned errors/advisories and scoped
verification results. Reuse attached state for recovery. A missing/failed snapshot is not permission to
repeat a mutation. For timeout/unknown outcome, inspect current state before
another mutation; use a request ID when a consequential action needs idempotent
recovery, not for every local field edit.

Keep recovery within the authorized task. Do not change login-session
environment or restart an app to work around a missing control. Report blocked
when recovery needs setup or permissions outside that scope. See
[recovery.md](references/recovery.md) for command inspection and error codes.

## Complete The Requested Result

At the final group boundary, compare the requested edits with the returned
content and the app's saved/modified state. Content change alone does not prove
saving. Once these requirements are satisfied, finish; do not repeat Save,
reopen correct documents or reread unchanged state merely to seek a stronger
verification flag or an exact AX/file-byte match.

Investigate only a specific unresolved requirement. Use evidence that can answer
it; repeating the same ambiguous AX value cannot establish a file-byte property.
Correct a demonstrated content error, then recheck that affected result. If the
authorized interface cannot resolve the remaining uncertainty, state precisely
what is complete and unverified and stop, without claiming unobserved success.

## Output And Further Commands

The helper defaults to concise JSON and complete snapshots up to 200 elements,
including unchanged controls and full values. Increase `--limit` only when a
truncation hint says needed content is missing. `--compact`, `--json` and
`--human` before the command retain task binding; `--json` adds geometry/axPaths,
not a different state model.

Use `state` when window/display metadata or an image is needed. File outputs need
an absolute task-owned output directory; read returned paths through the host's normal file capability.

Use `./pilot <command> --help` for unfamiliar syntax when using the launcher. Read only the relevant reference:

- [commands.md](references/commands.md): CLI options and command inventory.
- [visual.md](references/visual.md): screenshots, OCR, coordinates and file output.
- [permissions.md](references/permissions.md): installation and permission errors.
- [embedding.md](references/embedding.md): packaging the skill for an Agent host.
