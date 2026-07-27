# macOS Permissions

Use this reference after `permission_denied`, missing state, or failed capture.

## Permission Scope

Computer Pilot uses separate macOS permission classes:

- Accessibility: AX observation and native AX actions.
- Screen Recording: screenshots and OCR.
- Automation: Apple Events, granted separately for each target app and used by
  `cu tell` plus any remaining System Events fallback.

There is no single global Automation-ready state. `cu setup` reports
Automation as `per_target_app` and `not_checked`.

## Diagnose

```bash
cu setup
```

Interpret the result:

- `ready:true`: core Accessibility and Screen Recording are available.
- `accessibility:false`: enable the signed Computer Pilot identity under
  Privacy & Security > Accessibility.
- `screen_recording:false`: enable it under Privacy & Security > Screen
  Recording.
- `capture_protected_apps`: these apps opt out of capture even when Screen
  Recording is granted.

The first `cu tell <target>` may trigger an Automation prompt for that target.
Grant only when the task needs Apple Events access.

## Recovery

After changing a TCC permission, quit and restart the controlling Computer
Pilot process if macOS does not refresh access immediately, then rerun
`cu setup` or the target command.

Do not describe Automation as a one-time global permission. Finder, Calendar,
System Events, and every other Apple Events target can have independent state.

Do not attempt to bypass capture-protected windows. Use AX-only operation and
manual visual confirmation where necessary.

## Distribution Identity

Bundled Agent products must execute the official signed/notarized Computer
Pilot artifact with its fixed identifier. Do not re-sign, wrap, or copy the
binary under changing identities; macOS TCC continuity depends on stable code
identity across upgrades.
