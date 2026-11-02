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

Choose the cheapest suitable interface. For a known app and a document, file,
or other app-data task, start with one read-only `cu tell` as the capability
probe; do not call `apps`, `sdef`, help, snapshot, or screenshot first. If that
probe succeeds and the operation is clear, group the authorized reads and
writes in one app-scoped script and make one final read. If it fails with an
unknown target, permission, or terminology error, use `cu apps` or `sdef` to
route the fallback. For UI-only work, use AX; start with a snapshot and use
screenshots/OCR only when AX lacks the needed content.

When the launcher already binds an exact `pid:<PID>`, do not call `cu apps` to
reconfirm that target; go directly to `snapshot` or `find`. Use `cu apps` only
when the target selector or running process is unknown.

When the task permits app-native scripting, this data route takes precedence
over a generic request to use the app UI: use AX only for presentation-specific
work such as menus, geometry, selection, or visual proof. Do not discover or
edit app data through clicks when the permitted `tell` route can address it.

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
When text begins with `-`, put all flags before the optional `--` separator:
`./pilot type --no-snapshot --no-verify -- '- [x] done'`. The CLI also accepts
a leading-hyphen text value directly; flags after `--` are always treated as
text.

Use only documented option names. `cu click` uses `--double-click` for a
double click and has no `--count` option. `cu find` filters with `--role`,
`--title-contains`, `--title-equals` and `--value-contains`; it has no `--text`
predicate. If a syntax error occurs, read that command's `--help` once and
retry with the documented form. Do not invent a flag or repeat the same invalid
command.

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

Keep a group within one observed UI state. Split it before navigation, menu
opening or closing, focus changes, selection changes, or any action whose
result changes the next target. `--no-snapshot` is appropriate for deterministic
local edits after the target is known; it is not a substitute for observing a
new selection, caret, window, or document section.

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

An action error is a branch point, not a retry signal. After a nonzero command,
`verification_failed`, `unknown_outcome`, or `stale_observation`, never rerun
the identical command or reuse the same ref/coordinates. Use the attached
`diagnostics.snapshot` when present; otherwise perform one fresh `find` or
`snapshot`, then choose a different target or method. If the same intended
effect fails twice, stop the loop and report the concrete blocker.

If AX exposes the intended control, recover with its fresh ref or `find` result;
do not switch to screenshot coordinates or OCR for the same control. Use
coordinates/OCR only when AX cannot expose the target.

Treat `focus_failed` as a foreground delivery blocker. Do not continue with
keyboard or typing commands, drop `--app`, or use global input after it. Inspect
the returned target state once; if the workflow cannot use a background-safe
primitive, report the concrete focus blocker instead of claiming completion.
Likewise, a `verification_failed` result with `focused:null` does not establish
input focus; do not send the text again until a fresh focus result exists.

For UI-only work, use the shortest observation that still identifies the next
target. If the role and label/value are already known, use one
`find --first` and act on its returned ref; use a full `snapshot` only when the
target or navigation is unknown, several candidates must be disambiguated, or
the task requires a complete visual state. Do not call `window focus` before
PID-targeted observation or actions. Those commands are background-safe; focus
only when the workflow explicitly needs foreground menus or keyboard handling.

## Observe Transitions

Choose the group's final observation from what is already known:

- Unfamiliar navigation: `snapshot` to discover what opened, including dialogs.
  For a known control, use `find --first` instead of a preliminary full tree.
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
next state. If the destination content is already present, use its controls;
focus can remain on the initiating row or link after navigation. Do not repeat
the accepted activation merely to move focus. Do not add fixed sleeps.

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

The click reports `focus_verified` for textfield-shaped targets, but a false or
unknown value does not fail the click: Finder filenames and similar controls
can be selectable rows rather than edit fields. `cu type` performs the strict
focused-input check immediately before dispatch and refuses to send text
without it.
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

At completion, reconcile the original request with observed results: every
requested change, preservation constraint, and required save/export. A successful
action group establishes only that group's result. Reuse the returned observations
for this comparison; obtain new evidence only for a specific missing or uncertain
requirement.

`effect_verified:null` means the CLI made no generic task judgment. Decide from
the requested outcome, not that flag. Foreground/window checks diagnose missing
effects, not additional completion requirements. Once every requirement is
supported, finish without repeating Save, reopening correct documents or rereading
unchanged state. Correct demonstrated errors and recheck only affected requirements.
If the authorized interface cannot resolve remaining uncertainty, state precisely
what is complete and unverified, without claiming unobserved success.

## Output And Further Commands

The helper defaults to concise JSON and complete snapshots up to 200 elements,
including unchanged controls and full values. Increase `--limit` only when a
truncation hint says needed content is missing. `--compact`, `--json` and
`--human` before the command retain task binding; `--json` adds geometry/axPaths,
not a different state model.

Use `state` when window/display metadata or an image is needed. File outputs need
an absolute task-owned output directory; read returned paths through the host's normal file capability.

Use `./pilot <command> --help` only when syntax is genuinely unfamiliar or
ambiguous after an error; do not call help before a known command. Read only the relevant reference:

- [commands.md](references/commands.md): CLI options and command inventory.
- [visual.md](references/visual.md): screenshots, OCR, coordinates and file output.
- [permissions.md](references/permissions.md): installation and permission errors.
- [embedding.md](references/embedding.md): packaging the skill for an Agent host.
