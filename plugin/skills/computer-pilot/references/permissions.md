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
- `accessibility:false`: enable the current Computer Pilot executable (or its
  signed identity) under Privacy & Security > Accessibility.
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

Inspect `release-index.json` before bundling and match its `signing.status`
against the manifest's `installation.signing.required_status`; a release that
contradicts the declaration is not an official artifact. Never re-sign an
official binary — that replaces its identity.

`ad-hoc-unsigned` artifacts carry the fixed identifier but no stable code
identity (an ad-hoc designated requirement is a bare cdhash that changes with
every build), so their integrity rests on the published SHA-256 digests.
`developer-id-notarized` adds Apple-verifiable provenance.

Signing tier does not by itself decide permission continuity: the grant
follows `tcc_subject`. When macOS attributes cu's checks to a responsible
process — the usual case in an Agent or terminal shell — the permission lives
on that host app and is unaffected by replacing the cu binary.
