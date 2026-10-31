# Agent End-to-End Tests

Real LLM agent tests for `cu`. Unlike `tests/commands/` (which test "does the command work"),
these tests verify "can an agent complete a real task correctly".

## Architecture

```
Task Definition          Agent Loop              Verification
┌──────────────┐    ┌─────────────────┐    ┌──────────────────┐
│ task.json    │───>│ LLM + cu calls  │───>│ verify.sh        │
│  - goal      │    │  observe → act  │    │  check actual    │
│  - setup     │    │  → verify loop  │    │  app state via   │
│  - verify    │    │                 │    │  cu tell / cu    │
│  - cleanup   │    │                 │    │  snapshot         │
└──────────────┘    └─────────────────┘    └──────────────────┘
```

Key difference from command tests: **verification checks the actual app state**,
not just whether `cu` returned `ok: true`.

## Running

```bash
# Run all agent tests
python3 tests/agent/run.py

# Run a specific test
python3 tests/agent/run.py --task tests/agent/tasks/mail_to_notes.json

# Dry run (show tasks without executing)
python3 tests/agent/run.py --dry-run
```

## Latency comparisons

The loop accepts a deterministic group of commands in one model response and
stops the group on the first failed command. It does not enumerate apps every
round, truncate command JSON, or add sleeps between commands. Use `cu wait`
when the task requires an asynchronous condition.

Results include `timings` (model calls, commands, model time, command time,
and total agent-loop time) and a per-step `trace`. Setup, verification, and
cleanup are outside the agent-loop timer. Compare the same task and model;
set `AGENT_REASONING_EFFORT` when the OpenAI-compatible provider supports it.
Leave it unset to use the provider default. This records actual latency rather
than inferring speed from token counts or Rust microbenchmarks.

For a longer, reproducible UI workflow:

```bash
python3 tests/agent/benchmark_long_workflow.py /path/to/baseline/cu target/release/cu \
  --baseline-ref b9987a6 --candidate-ref HEAD --repeats 3 --reasoning low
```

This launches an owned native app, enters six orders from visible source data,
sets quantity-dependent priorities, saves two corrections, and archives the
records. An independent check compares every original/final field and the saved
archive state. `--smoke` runs the fixture through a deterministic UI script
without model calls. The fixture is terminated after each trial.

Both arms use the same current harness, model, and reasoning setting, with their
respective CLI binary and committed skill. Trials alternate before/after order;
setup and independent verification are excluded from the agent-loop timing.
Results, complete transcripts, and binary hashes are saved under
`test-results/long-workflow/`. Failures remain in the results; successful-trial
medians alone do not establish reliability. This measures a native UI workload,
not Codex's private implementation or browser/Electron performance.

Measured on 2026-10-31 with `gpt-5.6-sol`, reasoning `low`, comparing
`b9987a6` against `1f3d307` (three interleaved trials each):

| Trial | Before | After |
| --- | --- | --- |
| 1 | 130.4 s, exact match | 82.3 s, exact match |
| 2 | 149.3 s, missed priority on order 6 | 142.2 s, omitted a character in two memos |
| 3 | 213.9 s, exact match | 47.8 s, exact match |

Including the failed trials, median agent-loop time fell from **149.3 s to
82.3 s (45% less time)**, model calls from **49 to 16**, and cumulative API input
tokens from **2,203,430 to 246,134**. Median command time changed from 8.1 s to
7.0 s; the main reduction was model time (141.2 s to 75.3 s). These medians
describe six observed runs, not a general speed guarantee or a cost estimate.

Exact task success was **2/3 in both arms**. The old agent omitted a checkbox
action; the new agent generated two memo strings missing `清` from `装箱清单`.
The CLI saved the supplied strings correctly. Both agents declared completion,
so dispatch success and frequent per-action snapshots did not establish task
correctness. The benchmark exits nonzero when any independent check fails.
See [machine-readable results](benchmarks/native-orders-2026-10-31.json) for
binary hashes, all timings, command errors, and exact field mismatches.

## Built-in Computer Use tool comparison

The same task was also executed through Codex's actual `cua_repl` native App
API and through the optimized `cu` CLI. Both arms ran a deterministic script
that copied live AX source values. Each trial made 32 field writes, 15 clicks,
and 20 state observations at identical group boundaries. There was no model
inference inside either measured interval; these times must not be compared
directly with the agent-loop times above.

| Tool execution, three-trial median | cu | Built-in cua_repl |
| --- | ---: | ---: |
| Complete workflow | 7.33 s | 16.36 s |
| State observations | 2.08 s | 13.04 s |
| Field writes | 2.53 s | 1.62 s |
| Clicks | 2.76 s | 1.76 s |
| Exact task success | 3/3 | 3/3 |

All high-level calls were timed, including CLI process/Broker overhead. Native
binding and a matching CLI warm-up observation were excluded, along with fixture
setup, independent verification, and cleanup. Native observations used their
default diffs after the initial full tree. CLI writes used `--no-snapshot`, and
clicks also used `--no-verify`; explicit snapshots read the same group boundaries.
Ref freshness checks remained enabled. Neither timed arm added sleeps.

The built-in API was faster at mutations, while state observation dominated its
total time. Its documented behavior includes waiting for UI updates; this test
does not separately measure settling versus AX traversal. The fixture updates
synchronously, so this result does not establish performance or reliability on
asynchronous apps, browsers, or a complete Codex model-driven session.

Reproduction uses the same owned fixture and independent field checker:

```bash
python3 tests/agent/benchmark_builtin_tools.py /tmp/cu-tool-comparison/session.json init
python3 tests/agent/benchmark_builtin_tools.py /tmp/cu-tool-comparison/session.json start builtin-1
```

In the real Codex `cua_repl`, bind the returned `app_path` with `cua.getApp()`.
Load the definition from [builtin_order_workflow.js](builtin_order_workflow.js)
into that REPL and call `await runNativeBenchmark(app)` in a separate call.
The returned object contains per-operation timings and observation traces.
Then run `SESSION verify` through the Python helper. For a fresh CLI trial, run
`SESSION start cu-1`, followed by `SESSION cu`; finish with `SESSION stop`.
Alternate arms and repeat three times. The helper does not recreate or expose
the private built-in API to an external model.

[Recorded results](benchmarks/builtin-tools-2026-10-31.json) include all three
samples per arm, per-method timing arrays, fixture/binary hashes, controls, and
independent verification. Native parser/REPL calibration failures occurred before
the measured series and are explicitly excluded from that report.

## Actual Codex agent comparison

Two isolated Codex tasks also completed the workflow using their ordinary agent
loops: one through the actual built-in native App API, the other through the
current Computer Pilot skill and shell. Each task retained its own history for
three sequential trials; repetitions are warmed runs, not fresh contexts.
Actual per-turn metadata recorded `gpt-6-astra`, reasoning `high`, and no explicit
service-tier override. These samples are separate from both comparisons above.

Initialization (tool documentation/skill reading and one app observation) was
untimed. Each subsequent `GO` turn used a fresh fixture process. Timing runs
from Codex's task-start to task-complete event, including the model and tools;
parent polling delay, fixture setup, and independent field checking are excluded.
The tasks received identical workflow instructions and could batch known actions
or generate code, but could not read fixture source/saved data, other tasks,
previous results, or prewritten solvers. No trial received correctness feedback.
Only one participant acted at a time, in order: native-1, cu-1, cu-2, native-2,
native-3, cu-3.

The offline collector reads only the selected task rollouts. It counts unique
response usage records, sums their token usage, and unions recorded shell/MCP
execution intervals. The remaining time includes model, network, and scheduling;
it is **not** a direct measurement of inference time. Tool calls here count
shell batches or REPL invocations, not individual clicks/field writes. Cumulative
input tokens include repeated cached context and are not a cost estimate.

To collect an owned experiment, place its two task IDs in `threads.json`
(`{"cu": "...", "native": "..."}`) beside each `LABEL-saved.json`, then run:

```bash
python3 tests/agent/collect_codex_benchmark.py /path/to/session \
  "$HOME/.codex/sessions/YYYY/MM/DD"
```

The collector writes `results.json` and local per-trial tool traces without
reasoning text. Use `benchmark_builtin_tools.py SESSION start LABEL` to reset
the owned fixture before each GO turn and `SESSION stop` after the series. Do
not use that helper's deterministic `cu` action for model-driven trials.

Measured results (2026-10-31):

| End-to-end trial | cu | Built-in native App API |
| --- | ---: | ---: |
| 1 | 180.4 s | 97.9 s |
| 2 | 129.8 s | 81.0 s |
| 3 | 136.1 s | 85.0 s |
| Median elapsed | **136.1 s** | **85.0 s** |
| Median recorded tool time | 4.51 s | 8.77 s |
| Median time outside tool intervals | 131.75 s | 77.01 s |
| Model calls per trial | 19 / 13 / 13 | 13 / 13 / 13 |
| Median returned text characters | 38,959 | 8,926 |
| Median cumulative input tokens | 917,216 | 530,735 |
| Median cached input tokens | 897,664 | 485,376 |
| Median model output tokens | 3,328 | 1,389 |
| Exact task success | 3/3 | 3/3 |

The native arm used **38% less median elapsed time** (cu took 1.60x as long),
despite longer recorded tool execution. This reproduces the product-level speed
difference on this workload and locates it mainly outside tool execution.
Component medians need not sum to the elapsed median.

Trace review found six extra pre-save observation batches in cu-1. The tested
skill warned against submitting unverified input, whereas the native agent grouped
field writes and Save. In cu-2/cu-3, the agent also grouped writes and Save,
reducing its model calls to 13, while still taking 129.8/136.1 s. These later
trials treated the local Save as batchable. This is observed strategy variation,
not a controlled experiment proving the effect of that skill wording. All trials
used the assigned backend and live UI only, and
all saved original fields, corrections and archive state passed independent
checks. No tool errors were recorded.

The remaining observed differences include repeated shell environment/argument
text, verbose CLI metadata/acknowledgments, and growing task history. CLI action
snapshots already returned ref deltas; the initial full snapshot and metadata
still made the payload larger. Median returned text was 4.36x larger and model output tokens 2.40x
higher for cu. These are optimization targets, not measured causal shares of
the remaining latency: model output generation, context processing, networking,
and scheduling were not isolated. Removing millisecond-scale native checks
alone cannot account for this observed gap. Future changes should shorten the
agent interaction and remove unnecessary observation rounds, then repeat the
same end-to-end measurement while retaining exact outcome checks.

[Recorded end-to-end results](benchmarks/codex-e2e-2026-10-31.json) contain all
samples, setup timings, actual model metadata, usage, checks and evidence hashes.
Only three trials per arm were run, with identical source records and retained
history; this does not establish performance for browsers, Electron, asynchronous
apps, fresh contexts, or other models. These initial measurements preceded the compact-output and skill changes below.

## Compact workflow after the fix

The candidate at `0136c8b` adds `--compact`, reuses Broker deltas for explicit
compact snapshots, removes unused geometry/display/cache work on that route,
and supplies a task-local shell helper. The skill groups native field edits and
an authorized Save/Next before one boundary observation; it no longer asks for
an extra pre-save model round. Ordinary JSON and error/recovery contracts remain
unchanged. Full snapshots and exact field values remain available.

The first attempt reused the earlier task histories. Native took 91.4 s;
cu took 166.6 s while reverting to its old verbose action template, then 178.7 s
with the helper after explicit setup guidance. That latter run included one
self-corrected BSD `sed` error and returned only 16,358 text characters, but
its 14 model calls cumulatively consumed 1,687,564 input tokens. All three
outcomes were exact. Reduced output did not speed up that existing long-context
task. These samples and setup turns are retained in the result artifact.

The final comparison starts two fresh Codex tasks using the same recorded
`gpt-6-astra / high` settings. The cu setup explicitly initializes the multi-round
configuration described by the current skill; neither participant receives a
solver or expected data. The native setup loads the actual App API. Both observe
once during untimed initialization, then run three interleaved GO turns with
fresh fixture processes, retaining their respective history between trials.
Fresh-PID configuration changes are timed. There is no timing/correctness feedback
between trials. This measures the configured workflow, not guaranteed migration
of an existing task away from its old command templates.

| Fresh-task series | Compact cu | Built-in native App API |
| --- | ---: | ---: |
| Trial 1 | 59.7 s | 70.8 s |
| Trial 2 | 65.8 s | 64.4 s |
| Trial 3 | 75.1 s | 70.4 s |
| Median elapsed | **65.8 s** | **70.4 s** |
| Median recorded tool time | 4.50 s | 7.79 s |
| Model calls per trial | 13 / 13 / 13 | 13 / 13 / 13 |
| Median returned text characters | 16,284 | 8,926 |
| Median cumulative input tokens | 558,226 | 520,015 |
| Median model output tokens | 1,260 | 1,196 |
| Exact task success | 3/3 | 3/3 |

The configured CLI workflow is now comparable with the contemporary native
control on this fixture. Against the earlier recorded CLI series, median
returned text fell **58%** and model output tokens **62%**; observed elapsed
changed from 136.1 to 65.8 s. That historical time difference is not a randomized
causal estimate: the native control also changed from 85.0 to 70.4 s, and
contexts, prompts, setup and backend load can affect latency. The remaining
per-trial ranges overlap. No inference-speed or general app-speed guarantee is
implied, and these changes do not erase existing task history.

[Complete post-fix results](benchmarks/compact-workflow-2026-10-31.json) include
all six fresh-series samples, all three continued-history diagnostics, actual
settings, setup timings, exact outcome checks and evidence hashes. Tool traces
confirm the assigned backends and live UI use; no expected/source/saved files or
prewritten solvers supplied action values. The fresh series had no tool errors.

Validation for this implementation: 43 Rust tests, clippy, release build, skill
validation, version synchronization and 82 command-test assertions passed.
Two existing TextEdit-dependent cases were skipped to preserve the user's open
session. The owned fixture test covers long exact Unicode values, compact delta
reconstruction, unchanged-ref use, fail-fast shell grouping, complete errors,
and unchanged ordinary JSON geometry/path output. Private Broker protocol 5
ensures old Brokers cannot silently omit compact explicit-snapshot deltas.
