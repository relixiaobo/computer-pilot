# Formal API L2 — 2026-11-06

The current local `codex/action-limit-audit` candidate ran the unchanged formal
API runner against all three release tasks. **2/3 verified; exit status 1.**
The later mail-only rerun passed **1/1** with unchanged tested sources and binary.
Every release task now has a formal API pass across the two runs; the initial
full run remains 2/3, not a successful single full cohort. This is actual API
execution, not external-agent evidence.

Configuration: project `.env` selected `gpt-6.1-terra` through the
OpenAI-compatible route; credential preflight passed. The default 25-step
budget was retained, with no model override or reasoning-effort override.
`cargo build --release` completed before the run. Source and binary SHA-256
digests were captured before execution and rechecked unchanged afterward.

| Task | Result | Steps | Formal checks |
| --- | --- | --- | --- |
| Calendar event from reminder | Passed | 6 | 2 passed |
| Desktop inventory note | Passed | 5 | 2 passed |
| Mail summary note | `setup_failed` | 0 | Verification-source capture failed |

All three tasks completed their cleanup path without a reported cleanup error.
There were no skipped verification checks. Reported usage was **58,614 tokens**
(57,567 input, 1,047 output), across 11 model calls for the two executed tasks.

## Mail blocker

Before the mail agent started, the runner attempted to freeze the subjects of
the three newest inbox messages for subsequent verification. Its `cu tell Mail`
source command timed out after 45 seconds (`command_expired`). No model request
or agent action ran for this task, so this is an unavailable test prerequisite,
not a failed agent summary or a provider quota error.

A follow-up read, `cu tell Mail 'get count of accounts' --timeout 10`, also
timed out. Mail's window list and AX snapshot were readable. This establishes
that a simple scripting read remained unavailable; it does not establish why.
The failed prerequisite was not bypassed, the task was not counted as passed,
and Mail was not restarted or foregrounded.

## Mail recovery and targeted rerun

A later identical account-count query returned successfully in approximately
0.1 seconds. No code or configuration change, Mail restart or foregrounding
occurred between the failed diagnostic and this successful read. The unchanged
formal runner was then invoked with
`--task tests/agent/tasks/e2e/mail_to_notes.json`, retaining the configured model
and default step budget.

The source capture succeeded. The agent completed in **10 steps**, all **3
formal checks passed**, cleanup reported no errors, and the runner exited 0.
Usage was **83,443 tokens** (81,776 input, 1,667 output). All recorded source and
binary hashes still matched the first run.

This supports a transient failure in the Mail scripting path rather than an
observed regression in the action-limit repair or an agent-task failure.
It does not isolate the ultimate cause within Mail/macOS Apple Events/the
invocation lifecycle. The timeout was raised while waiting for `osascript`,
not during a model call or a Broker queue wait. The CLI timeout/error handling
and the runner's refusal to grade an unavailable source behaved as intended.
Future stability work should first reproduce cold-versus-ready scripting
behavior; any bounded readiness retry belongs to the read-only prerequisite,
not arbitrary mutation replay or weaker verification.

## Evidence

Ignored local run directory: `test-results/l2-2026-11-06.VfYTsK/`.

- `runner.log`: complete runner output.
- `results.json`: copy of the original `test-results/agent-1793932359.json`.
- `summary.json`: task status, counts, token usage and unchanged-source check.
- `source-hashes.json`: CLI, dependency, packaged Skill, task, oracle and binary hashes.
- `exit-code.txt`: `1`.
- `mail-retest.log`, `mail-retest-results.json`, `mail-retest-summary.json`:
  targeted pass; original result is `test-results/agent-1793933775.json`.
- `mail-retest-exit-code.txt`: `0`.

The local raw traces contain actual application data and are not copied into
this tracked report. The binary SHA-256 is
`b501b33d07352c0b0a7d5332939ae69f83eb2ddccf9f193e994eb1f5d2835329`.

The standard checks have their existing scope: calendar checks establish the
title match; the inventory cross-check uses source tokens. The mail checks cover
the note title, exact captured subjects and minimum content length. These runs
do not imply additional exactness checks or full acceptance
of every detail in the task goals. See the separate
[action-limit acceptance report](2026-11-06-action-limit-audit.md) for local
behavior tests, actual-app limits and the repaired advisory regression.
