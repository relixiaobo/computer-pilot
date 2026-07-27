# Scripting Workflow

Use this reference when the target app exposes an AppleScript dictionary or
the task is data-oriented rather than UI-oriented.

## Select Scripting

```bash
cu apps
cu sdef "Calendar"
```

Use `cu tell` when the app has `scriptable:true` and the requested object or
operation appears in `cu sdef`. Use AX when the object exists only in the UI,
the app is not scriptable, or AppleScript fails.

Scripting is usually preferable for bulk reads/writes because it expresses one
semantic operation without UI traversal.

## Execute

`cu tell` wraps the expression in an escaped target-app tell block and applies
a timeout:

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

For mutations, assign a request ID and verify by reading the object back:

```bash
cu --request-id "calendar-review-20260728" tell Calendar 'make new event at end of events of first calendar with properties {summary:"Review"}'
cu tell Calendar 'get summary of every event of first calendar'
```

## Automation Permission

Automation is granted separately for every target app. `cu setup` cannot report
one global Automation boolean. The first `cu tell <app>` may trigger macOS TCC;
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
