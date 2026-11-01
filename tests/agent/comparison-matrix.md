# Codex and Computer Pilot comparison matrix

This matrix compares general desktop ability across six task classes. It keeps
the task goal and independent oracle identical between the two arms; only the
backend changes:

- **Codex built-in Computer Use:** the native `cua` App APIs and their returned
  accessibility state.
- **Computer Pilot:** the existing shell, `cu`, and the packaged
  `computer-pilot` skill.

The cases are deliberately split by failure mode. A single long editor task can
show a speed difference, but it cannot tell whether a problem came from target
disambiguation, exact text preservation, cross-app transfer, or recovery after
UI drift.

Each trial uses a fresh task-owned window or process, task-owned files/data, and
an independent verifier. Setup, teardown, fixture compilation, application
launch, and the verifier are outside the primary task timer. The timer starts at
the first task observation and ends when the agent reports completion or a
concrete blocker. The verifier then records correctness without coaching or
repairing the task.

Run three repetitions per case, randomize arm order, and keep the model,
reasoning effort, service tier, host, display, and application versions fixed.
Use a fresh model profile, cwd, runtime directory, and output directory for
every trial. Disable project-document injection and memories; the Computer Pilot
arm receives only its packaged skill, and the native arm receives only the
native Computer Use documentation. Do not let `AGENTS.md`, `CLAUDE.md`, prior
conversation state, or a previous trial enter either arm.

Record these fields for every trial:

| Field | Meaning |
| --- | --- |
| `verified` | Independent exact oracle passed |
| `agent_status` | Done, blocked, failed, or budget exhausted |
| `elapsed_ms` | First observation through the final agent result |
| `model_turns` | Number of model responses |
| `backend_calls` | CUA calls or `cu` invocations |
| `backend_errors` | Calls that returned an error or nonzero exit |
| `recovery_calls` | Calls after a stale, focus, verification, or unknown-outcome failure |
| `post_exact_calls` | Calls after the verifier first observed the exact final state |
| `input_tokens`, `output_tokens` | Model usage, when the host exposes it |

Accuracy is reported before speed. A trial that is not independently verified
is a failure and is excluded from successful-trial speed medians. Report both
all-trial medians and successful-trial medians; never convert a failed fast run
into a speed win. Report false completion separately when the agent says it is
done but the oracle fails.

Neither arm may use direct document filesystem edits, an external automation
script, a browser or DOM backend, or a task-specific solver. The Computer Pilot
arm may use documented `cu tell` for app-native data because that is part of its
public interface. The native arm may use only the native Computer Use APIs. Both
arms may batch actions that follow from their current observation, but neither
may use fixed sleeps, precomputed target coordinates, hidden expected values, or
the verifier as feedback.

The first pilot should run the six cases in
[codex_comparison_matrix.json](tasks/codex_comparison_matrix.json). Existing
editor and Finder fixtures can supply the first three cases; the owned order
fixture supplies dynamic recovery. The cross-app and stale-target cases need
fresh task-owned fixtures so their setup and cleanup are symmetrical.
