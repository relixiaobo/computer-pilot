# Recovery And Reliability

Use this reference after a failed, stale, cancelled, expired, unverified, or
uncertain command.

## Command Identity

Set a stable client key for one logical Agent. Add a request ID to mutations
that may need recovery:

```bash
export COMPUTER_PILOT_CLIENT_KEY="agent.invoice-workflow"
cu --request-id "submit-invoice-4821" click 9 --app Acme --observation "<id>"
```

Reusing the same request ID and identical command replays its recorded result.
Reusing it for different arguments returns `request_id_conflict`.

## Inspect Commands

```bash
cu status
cu commands --limit 20
cu command "<command_id>"
cu cancel "<command_id>"
```

Command states include `accepted`, `dispatched`, `completed`, `cancelled`,
`expired`, and `unknown_outcome`.

- `accepted`: queued; mutation has not been dispatched.
- `dispatched`: child execution started.
- `completed`: inspect exit code and result.
- `cancelled`: known not to have completed as a read operation.
- `expired`: deadline elapsed before or during a non-mutating command.
- `unknown_outcome`: a mutation was dispatched but completion is uncertain.

On `unknown_outcome`, observe or read current state. Never replay the mutation
solely because the caller timed out.

## Stable Error Codes

| Code | Meaning | Default response |
|---|---|---|
| `invalid_argument` | Invalid CLI or Broker input | Correct the request. |
| `permission_denied` | TCC or private authentication denied | Fix the relevant permission. |
| `app_not_found` | App missing or not running | Inspect `cu apps` or launch it. |
| `ambiguous_target` | Name/bundle matched multiple processes | Choose `diagnostics.candidates[].selector` and reuse that PID selector. |
| `window_not_found` | Target window unavailable | Observe/launch and wait for a window. |
| `observation_required` | Ref has no current client Observation | Run `cu state` or `cu snapshot`. |
| `observation_not_found` | Observation missing, foreign, or expired | Observe again. |
| `stale_observation` | UI/window/ref changed | Observe again and re-plan; do not dispatch old intent. |
| `target_busy` | Resource cannot be accepted immediately | Inspect commands and retry only if safe. |
| `capture_protected` | OS forbids screen capture | Continue with AX or manual verification. |
| `verification_failed` | Requested verification failed | Inspect current state and choose another primitive. |
| `request_id_conflict` | ID reused for different request | Keep original intent or use a new ID. |
| `command_in_progress` | Same request is active | Inspect its command ID. |
| `command_cancelled` | Read cancelled with known outcome | Re-run only if still needed. |
| `command_expired` | Read/queued command missed deadline | Re-run only after confirming relevance. |
| `unknown_outcome` | Dispatched mutation outcome unknown | Inspect state before any retry. |
| `internal_error` | Private implementation failure | Inspect status; do not assume mutation safety. |

Use `retryable` only as transport guidance. It never proves that replaying a
mutation is semantically safe.

## Stale Observation

On `stale_observation`:

1. Do not retry the same ref action.
2. Run `cu state <app>` or `cu snapshot <app>`.
3. Re-identify the intended control in the new Observation.
4. Dispatch with the new `observation_id`.

## Verification

For `verified:false`, read `verify_advice` and the attached snapshot. Prefer:

1. `cu set-value` for AX fields.
2. `cu perform <ref> AXPress` for an exposed native action.
3. `--ax-path` when structure is known.
4. A fresh ref from a new Observation.
5. OCR or visual targeting when AX is sparse.

Do not recover by dropping `--app` or using `--allow-global`.

## Method Routing

| Method family | Meaning |
|---|---|
| `ax-action`, `ax-set-value`, `ax-perform` | Direct AX operation; preferred. |
| `cgevent-pid`, `unicode-pid`, `key-pid`, `ocr-text-pid` | PID-targeted, normally non-disruptive. |
| `paste-pid` | PID-targeted clipboard paste; desktop clipboard lock used. |
| `*-global` | Global HID delivery; disruptive and focus-sensitive. |

Hover and drag move the real pointer even when PID-targeted.

## Advisory Strings

Always process fields ending in `_hint`, `_reason`, `_advice`, or `_error`.
Common fields are `truncation_hint`, `confidence_hint`, `paste_reason`,
`verify_advice`, and `screenshot_error`.
