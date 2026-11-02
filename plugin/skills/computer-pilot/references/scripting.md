# Scripting Workflow

Use this reference when the target app exposes an AppleScript dictionary or
the task is data-oriented rather than UI-oriented.

## Select Scripting

Use scripting when permitted and the needed app objects/operations are exposed.
For known terminology, make the first useful read with `cu tell`; do not add a
separate version probe. If terminology is unknown, read `cu sdef <app>` before
inventing expressions. Use `cu apps` when process identity or scriptability is
unknown. Skip this path for known unsupported operations and explicit UI-only
tasks. A scriptable flag or successful generic read does not prove all app data
is scriptable. Permission errors need permission recovery, not more discovery.

Scripting is usually preferable for bulk reads/writes because it expresses one
semantic operation without UI traversal. The session helper leaves `sdef` and
`tell` arguments unchanged. `sdef` accepts an app name or bundle identifier;
`tell` accepts an app name or bundle path, not `pid:<PID>`. It is not bound by
the helper's `CU_APP`; if the exact intended instance cannot be addressed
unambiguously, use PID-targeted UI commands. Address the intended document
explicitly when the app has multiple documents. Group authorized operations
and read the affected result once; reuse that evidence at completion.

## Execute

`cu tell` wraps the expression in an escaped target-app tell block. Its timeout
defaults to 10 seconds; use `cu tell App '<expression>' --timeout 30` for 30
seconds. The root option before `tell` instead sets the Broker deadline in
milliseconds. Keep document/table operations in their object's tell block and
application operations outside it:

```bash
cu tell Safari 'get URL of current tab of front window'
cu tell Finder 'get name of every item of front Finder window'
cu tell Notes 'get plaintext of note 1'
cu tell Reminders 'get name of every reminder whose completed is false'
```

Multi-line expressions are supported:

```bash
cu tell Calendar 'set d to (current date) + (1 * days)
set hours of d to 10
set minutes of d to 0
make new event at end of events of first calendar with properties {summary:"Review", start date:d, end date:d + (1 * hours)}'
```

For mutations that need idempotent recovery, assign a request ID and verify by
reading the object back. The helper accepts `--request-id` after the command:

```bash
./pilot tell Calendar 'make new event at end of events of first calendar with properties {summary:"Review"}' --request-id "calendar-review-20260728"
./pilot tell Calendar 'get summary of every event of first calendar'
```

Expressions containing `activate` or System Events `keystroke`/`key code` are
refused: they steal the user's frontmost app or type into whatever the user
has focused. Use PID-targeted `cu key`/`cu type` with `--app` instead. Do not
answer that refusal with `--allow-disruptive`; it exists only for workflows
that genuinely require user-visible foreground activation.

## Automation Permission

Automation is granted separately for every target app. `cu setup` cannot report
one global Automation boolean. `cu apps` reads local metadata only; `cu sdef`
may request dynamic terminology for a legacy dictionary. That request or the
first `cu tell <app>` may trigger macOS TCC;
read [permissions.md](permissions.md) when access is denied.

Do not replace `cu tell` with direct `osascript`. Direct calls bypass Computer
Pilot command identity, timeout handling, escaping, recovery, and Broker
coordination.

## Failure Handling

- On `permission_denied`, obtain Automation access for that target app.
- On `app_not_found`, confirm the installed/running app identity with `cu apps`.
- On `command_expired` before dispatch, retry only with a fresh request ID.
- On `unknown_outcome` after a write, read current app data before deciding.
- On unsupported terminology, inspect `cu sdef` and switch to AX if necessary.
- A script error can follow successful earlier writes. Read the affected objects
  before retrying a mutation group; a failed script is not a transaction rollback.
