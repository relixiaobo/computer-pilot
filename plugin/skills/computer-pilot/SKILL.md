---
name: computer-pilot
description: Read and operate macOS apps through the cu CLI and the Agent's existing shell, using app scripting, Accessibility, keyboard/mouse input, screenshots and OCR. Not needed for ordinary file edits.
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

## Choose The Control Path

Respect the requested interface: an explicit UI-only task stays on UI paths.
Otherwise choose the shortest path that exposes the required operation and
result. These are alternatives, not a mandatory discovery sequence.

| Task and available evidence | Path |
|---|---|
| App data or document/file operations with known scripting support | `tell`: make the first useful read, group authorized writes, and read the result. Use `sdef` only for unknown terminology; skip scripting for known unsupported apps/operations. See [scripting](references/scripting.md). |
| Controls, menus, dialogs, selection, or editing exposed by Accessibility | AX: reuse returned refs; otherwise `find` for a known target or `snapshot` for exploration. `click` already tries AX actions before mouse fallback; `perform` is for a specific action such as `AXShowMenu` or `AXScrollToVisible`. |
| A canvas, missing AX text/control, or appearance that must be seen | Visual: `ocr` for text; `screenshot` for layout/non-text targets; `snapshot --with-screenshot` or `--annotated` when both tree and image are needed. Use grounded `click --text` or screen coordinates. See [visual](references/visual.md). |
| Launching, arranging windows, or a known preference key | `launch`, `window`, or `defaults`. A preference write does not prove the app applied it; do not restart user apps merely to force a change. |

For Apple Notes creation or body replacement, read
[Notes title and body](references/scripting.md#notes-title-and-body): replacing
the first body line can change the note's name.

Within UI work, use `set-value` for replacing a native edit field's complete value.
An editable-looking label is not necessarily in edit mode. Use the app's editing
interaction to enter that mode; an AXValue write alone need not commit app data.
Use `type` for insertion/replacing a selected range and for Electron/CEF or
`AXWebArea` inputs; `type` automatically chooses Unicode or preserved clipboard
paste. Use `key` for known shortcuts, `scroll` to reveal content, and `hover` or
`drag` for pointer gestures. These are action primitives, not separate backends.
`type` requires a focused AX text input even on the visual path: an entirely
AX-less canvas cannot be typed into through the targeted `type` command.

Use `state` when window/display metadata is needed, with `--no-screenshot` if
no image is needed. `snapshot` and `find` suffice for ordinary AX work. Compact
and full JSON are presentations of the same observation, not two control paths.
For web-page DOM work, prefer Browser Pilot when available and permitted; browser
chrome, native dialogs, and desktop apps remain Computer Pilot tasks.

Use `apps` when process identity or scripting capability is unknown. Its
`scriptable` flag does not prove that a particular operation is supported.
An already supplied exact PID needs no rediscovery. A successful generic
`tell <app> 'get version'` likewise does not prove document scripting support.

## Keep The Task Binding

For repeated actions, reuse the supplied task launcher or create one in a
fixed task directory.
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
# Invoke the first command chosen above; there is no mandatory snapshot here.
```

The [helper](scripts/session.sh) supplies compact output and the fixed PID for UI
commands (including `window`); omit repeated `--app` or positional app selectors.
It preserves CLI verification and automatic snapshots. For known intermediate
actions, `--no-snapshot` omits their output tree; only `click`, `type` and `key`
accept `--no-verify`. Check the result at the group boundary. `tell` and `window`
accept neither flag. Other commands keep ordinary arguments:
`./pilot sdef "App Name"` accepts an app name or bundle identifier.
`./pilot tell "App Name" '<expression>'` accepts an app name or bundle path;
AppleScript does not accept `pid:<PID>`.
The helper does not bind `tell` to `CU_APP`. If exact process identity matters
and scripting cannot address that instance unambiguously, keep the work on
PID-targeted UI commands. For multiple apps, keep separate bound launchers or
pass each exact target explicitly using the same task client key.
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

Split a group when the next action needs information that the current state
does not establish: a new dialog, menu item, search match, or document range.
A known focus-and-type sequence or observed selection followed by a known
shortcut can stay together. Do not use counted arrows or an assumed caret to
guess an unknown selection. `--no-snapshot` omits output, not target identity
checks; only use it when the group's final observation covers the intended edits.

- Leave already-correct values, checkboxes and selections alone.
- For a dropdown that needs changing, open it and observe the menu. Select the
  observed item with `perform <ref> AXPress` (or its supported action).
  Observe the resulting controls before further edits or navigation.
  Writing the popup's AXValue may not select the item or fire its handler.

After an error, read whether dispatch occurred and reuse attached state.
A syntax error needs corrected syntax; a stale ref needs a fresh target;
an uncertain mutation needs observation before any retry. Do not repeat an
unchanged failed approach or force a different method merely because the ref
number is stale. Stop when no new evidence supports a safe next step.

Prefer a fresh AX ref for recovery. If AX is absent or demonstrably unusable,
use a current screenshot/OCR target; a visual fallback cannot bypass stale
identity, permission, or focus failures.

Treat `focus_failed` as a foreground delivery blocker. Do not continue with
keyboard or typing commands, drop `--app`, or use global input after it. Inspect
the returned target state once; if the workflow cannot use a background-safe
primitive, report the concrete focus blocker instead of claiming completion.
Likewise, a `verification_failed` result with `focused:null` does not establish
input focus; do not send the text again until a fresh focus result exists.

Reuse the last action's state when it already identifies the next target.
Otherwise use `find` for known role/label/value filters. `--first` selects the
first match in tree order, not the best match: use it only for an unambiguous
target and inspect `count`/`truncated`. For duplicate labels use the returned
candidates and context, a full JSON snapshot, or `observe-region`.
Use `snapshot` for unknown navigation. Do not call `window focus` before
PID-targeted observation or actions. Those commands are background-safe; focus
only when the workflow explicitly needs foreground menus or keyboard handling.

## Observe Transitions

Choose the group's final observation from what is already known:

- Unfamiliar navigation: use the last action's snapshot to discover what opened,
  including dialogs; request `snapshot` only if the needed state is missing.
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
PID binding selects a process, not a task window. Observations retain `window_id`
even in compact output; identical titles can belong to different windows.
After a window change or stale observation, match the returned window/document
context to the authorized task before using its refs. A fresh ref in another
window does not authorize work there.
Use `window list` if the intended window needs locating; focus its observed index
only when foreground work is needed. Closing windows is not a focus-recovery step.

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

When moving a section, account for separators at both its source and destination.
Cutting the last section from its heading can leave the preceding blank line at
EOF. Choose the range from the observed boundaries and check the resulting
spacing in the group's returned state, including the final newline.

AX editor focus does not make an app frontmost. Some apps ignore background menu
shortcuts such as Select All or Save. Prefer native scripting for document data;
when the workflow permits and needs foreground editing, explicitly focus the
intended window before the edit group.

For Electron/CEF or an `AXWebArea` input, use keyboard/type input instead of
AXValue writes. Omit the click when the intended editor is already focused.
For a new editor, a shell group can check the click's `focus_verified:true`
before typing, stopping on false/unknown without another model round.
Click and type use the application's focused-input observation; a true click
result establishes that clicked input at that moment, not continued focus or
foreground delivery. A successful click with false/unknown focus may have selected
a row instead of entering an editor. `type` checks for a focused text input
immediately before dispatch; the caller still chooses the intended editor.
`value_changed:true` reports an AX value change, not exact input or saving;
`effect_verified` remains null. A click's `verify_diff` likewise reports tree
changes; only `focus_verified` establishes the scoped input-focus result.
Virtual editors can expose a slice or synthetic boundary newlines in AX.
Treat AX text as an observation, not saved-file bytes: do not add/delete boundary
characters or rewrite correct content merely to match that representation.
An accepted AXValue/AXConfirm and a changed label can still leave application
data unchanged. Commit through the app's editing workflow and use its resulting
document/list/data state as evidence. When that evidence is absent, do not repeat
AX writes or declare completion from the same editable representation.
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

When the request specifies an output name/title, include it alongside content in
the final edit group's readback. A body-only read does not verify that identity.

For explicit whitespace constraints, compare literal line breaks in the returned
text with the original/requested boundaries; visually similar text is insufficient.
Use the same editor's observed representation consistently. If it cannot establish
the required boundary, report that uncertainty rather than claiming exact preservation.

`effect_verified:null` means the CLI made no generic task judgment. Decide from
the requested outcome, not that flag. Foreground/window checks diagnose missing
effects, not additional completion requirements. Once every requirement is
supported, finish without repeating Save, reopening correct documents or rereading
unchanged state. Correct demonstrated errors and recheck only affected requirements.
If the authorized interface cannot resolve remaining uncertainty, state precisely
what is complete and unverified, without claiming unobserved success.

## Output And Further Commands

The helper defaults to concise JSON; bare `./pilot snapshot` sets a 200-element
limit. With other snapshot options, pass `--limit` if the CLI default of 50 is
insufficient. Observations include unchanged controls and full values within
their limit. Increase `--limit` when truncation hides needed content.
`--compact`, `--json` and
`--human` before the command retain task binding; `--json` adds geometry/axPaths,
not a different state model.

File outputs need an absolute task-owned output directory; read returned paths
through the host's normal file capability.

Use `./pilot <command> --help` only when syntax is genuinely unfamiliar or
ambiguous after an error; do not call help before a known command. Read only the relevant reference:

- [commands.md](references/commands.md): CLI options and command inventory.
- [visual.md](references/visual.md): screenshots, OCR, coordinates and file output.
- [permissions.md](references/permissions.md): installation and permission errors.
- [embedding.md](references/embedding.md): packaging the skill for an Agent host.
