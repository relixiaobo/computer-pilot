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
- `permissions.<name>.granted:false`: follow that entry's `remediation`
  string. It names the exact subject to enable; `settings_url` is the
  System Settings deep link to give the user (JSON mode never opens System
  Settings itself).
- `permissions.<name>.granted:null`: **not probed — never read this as a
  denial.** Only `automation` reports null, because Automation is granted per
  target app and is requested by `cu tell`, not by `cu setup`.
- `tcc_subject.grant_subject`: the exact name to enable in System Settings —
  use this rather than assembling one yourself. It resolves to
  `responsible_process` when macOS attributes cu's checks to a host app (an
  Agent runtime or terminal), otherwise to the `executable` path.
- `tcc_subject_hint`: present only when neither could be resolved; the
  subject name is then a placeholder, so surface the hint instead of
  instructing the user to enable it.
- `capture_protected_apps`: these apps opt out of capture even when Screen
  Recording is granted.

The legacy top-level `accessibility` / `screen_recording` booleans remain for
compatibility; prefer the structured `permissions` object.

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

Inspect `release-index.json` before bundling. Prefer an official
`developer-id-notarized` artifact with the fixed identifier and do not re-sign
it. An `ad-hoc-unsigned` artifact is usable when signing credentials are
unavailable, but TCC continuity across upgrades is not guaranteed; pin its
checksum and surface that limitation to users.
