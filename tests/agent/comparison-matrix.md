# Codex and Computer Pilot comparison matrix

Status as of **2026-11-03**: the harness and initial reports shipped in v0.9.3;
the complete six-class, three-repeat matrix has **not** been executed. The
[evidence index](README.md#status-and-evidence-index) links the completed pilots
and cu-only follow-ups. Cross-app transfer, stale-target fixtures and explicit
same-label/multi-pane coverage remain future benchmark work, not release gates
already passed.

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
launch, and the verifier are outside the primary task timer. The timer covers
the complete participant turn, including skill/API discovery, through the
agent's completion or concrete blocker. The verifier records correctness without coaching or
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
| `elapsed_ms` | Complete participant turn, including skill/API discovery |
| `model_turns` | Number of model responses |
| `tool_calls` | Outer shell or CUA tool calls, including discovery; a call may batch several actions |
| `tool_errors` | Outer calls that returned an error or nonzero exit |
| `backend_calls` | Optional audited count of actually dispatched CUA calls or `cu` invocations |
| `recovery_calls` | Optional audited calls after a stale, focus, verification, or unknown-outcome failure |
| `post_exact_calls` | Calls after the verifier first observed the exact final state |
| `input_tokens`, `output_tokens` | Model usage, when the host exposes it |

Accuracy is reported before speed. A trial that is not independently verified
is a failure and is excluded from successful-trial speed medians. Report both
all-trial medians and successful-trial medians; never convert a failed fast run
into a speed win. Report false completion separately when the agent says it is
done but the oracle fails.

The current collector measures outer calls. Do not count command strings as
executed backend actions: an error can stop a batch before later actions run.
Its tool intervals and remaining turn time separate observed tool execution
from the residual model/network/orchestration interval; the residual is not a
measurement of model inference alone. Output character counts are not a fair
text-size comparison when one backend includes encoded screenshots.

Neither arm may use direct document filesystem edits, an external automation
script, a browser or DOM backend, or a task-specific solver. The Computer Pilot
arm may use documented `cu tell` for app-native data because that is part of its
public interface. The native arm may use only the native Computer Use APIs. Both
arms may batch actions that follow from their current observation, but neither
may use fixed sleeps, precomputed target coordinates, hidden expected values, or
the verifier as feedback.

The six planned cases are in
[codex_comparison_matrix.json](tasks/codex_comparison_matrix.json). Existing
editor and Finder fixtures can supply the first three cases; the owned order
fixture supplies dynamic recovery. The cross-app and stale-target cases need
fresh task-owned fixtures so their setup and cleanup are symmetrical.

The current Finder pilot tests navigation, selected file moves, and renaming;
it does **not** construct duplicate labels or prove multi-pane disambiguation.
Its oracle compares the complete regular-file set and contents (ignoring Finder's
`.DS_Store`), rejects symlinks, and catches copying instead of moving. Cleanup
uses each window's target URL because its title changes during navigation.

Run the existing real-app fixtures with a configured, authenticated Codex profile:

```sh
python3 tests/agent/benchmark_real_apps.py /absolute/new-run prepare --dataset finder --repeats 3 --seed 20261102
python3 tests/agent/run_codex_comparison.py /absolute/new-run --base-profile /absolute/cua-profile
python3 tests/agent/benchmark_real_apps.py /absolute/new-run report
```

Without `--repeats`/`--seed`, `prepare --dataset finder` creates one pair for each
of two cases with alternating arm order. Label such runs as pilots. The options
above freeze three repetitions and randomized order in the manifest before
launching participants. The runner executes serially because
both arms share the desktop. It creates fresh profiles and neutral working
directories, does not link personal skills, disables project docs and memories,
and retains rollouts and context audits. The supplied base profile provides host
configuration and authentication; keep it private. Audit actual instruction and
tool use before accepting a run. A fresh profile alone does not prove isolation.
Before each trial, the coordinator privately saves every clipboard item/type
and clears the pasteboard, outside task timing. This prevents unsupported Cut
shortcuts from pasting files left by an earlier trial. Cleanup restores the
backup only if the pasteboard is unchanged since clearing or contains only
owned fixture file paths; otherwise it preserves the changed clipboard.
Clipboard backups contain private data and must never be committed or exported.

Interactive diagnostics performed in the development conversation are excluded
from clean end-to-end comparisons. Deterministic backend replays measure API
latency only and must be reported separately. Do not combine either with fresh
participant runs or substitute them for failed attempts.

[textedit_holdout.json](tasks/textedit_holdout.json) is a new UI-only maintenance
handoff task: four scoped edits, a section move, and two checklist edits, while
preserving a similar archived section, literal ticket strings, and Unicode.
It uses different content from the earlier release-editing fixtures. Its exact
saved-file oracle is available only to the coordinator.
Run it with `prepare --dataset holdout`; the same runner and verifier apply.

The [2026-11-02 clean pilot](results/2026-11-02-clean-pilot.md) contains two
audited pairs and the exclusions from the earlier clipboard-contaminated cohort.
