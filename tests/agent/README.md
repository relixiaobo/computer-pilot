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

## Standard protocol for new comparisons

Use `benchmark_varied_workflows.py` for new **agent-level** cu versus built-in
Computer Use comparisons. The older experiments below remain historical records;
do not combine their samples with a new experiment. Deterministic tool replay
measures dispatch overhead separately and is not an agent completion-time result.

The first standardized block asks: after the session/wait changes, does cu still
take longer on ticket triage under the same model/settings? It uses four paired
datasets (eight fresh tasks), with native/cu and cu/native orders alternating;
its results appear below under “Standardized triage block”. Choose scope and
repeat count before seeing results. This is a diagnostic sample, not a statistical
guarantee. Evaluate this block before expanding to document/wizard.

```bash
cargo build --release
python3 tests/agent/benchmark_varied_workflows.py test-results/triage-v1 prepare \
  --modes triage --repeats 4 --model gpt-6-astra --effort high
```

`manifest.json` freezes the planned sequence, expected actual model/effort/service
tier, commit and binary/fixture/skill/helper/collector fingerprints. No tier
override is the default; use `--service-tier` only when both arms actually use
that tier. These options record the design; they do not configure or invoke a
model. `start` refuses changed experiment inputs. Do not edit the manifest, code
or skill during an experiment; use a new root for a new candidate.

For **each** manifest entry, sequentially:

1. Run `ROOT start LABEL`. Create one new task from the exact generated brief,
   using the planned settings, and save its ID in `ROOT/LABEL/thread.txt`.
   The app bundle/process, source/saved state, and cu Broker/client/output paths
   are fresh. No other participant runs during setup or execution. Do not supply
   a solver, source files, earlier outcomes or strategy coaching.
2. Let setup finish with READY and one initial observation, without UI mutations.
   Send exactly `GO cu-N.` or `GO native-N.` for that entry's dataset number N.
   Keep all in-turn errors and recovery. Do not restart a slow/failed attempt,
   provide hints, or add another GO to the same task.
3. When the participant completes or reports blocked, run `ROOT collect LABEL`,
   then `ROOT stop LABEL`. Collection reads only that task's rollout, exports
   **all setup and GO tool records** to `trace.json` (a list of turns), and checks
   saved fields, order and required original/correction states independently.
   It hashes the source, saved state, brief, task config when present, and trace.
   Stop also terminates the dedicated cu Broker; do not query status afterward
   because status starts a Broker when absent.
4. Review the complete trace once, offline, and save `ROOT/LABEL/audit.json`:

   ```json
   {"compliant": true, "note": "Reviewed setup and GO: fresh task, allowed backend, task environment retained on every app call, live UI sources only, no coaching."}
   ```

   For cu, inspect the sourced task configuration and every app call, not merely
   setup; for native, require the actual native cua API. Check that setup did not
   mutate the app and that tasks did not overlap. Mark any violation
   `compliant: false` with its concrete reason. This review is intentionally
   manual: matching a command string cannot prove shell environment isolation.
   A tool error, slow recovery, or wrong final value is **not** an exclusion.

Here `ROOT ACTION LABEL` abbreviates:

```bash
python3 tests/agent/benchmark_varied_workflows.py test-results/triage-v1 ACTION LABEL
# After all planned attempts, or to inspect incomplete coverage:
python3 tests/agent/benchmark_varied_workflows.py test-results/triage-v1 report
```

`report.json` and `report.md` retain every planned attempt. Missing results stay
pending; missing audits stay unreviewed. Model-setting mismatch, reused task,
changed/mismatched source data and explicit protocol violations are excluded
with reasons. Correctly conducted task failures remain failures, including their
elapsed time. An incomplete or extra GO is a protocol deviation, not a successful
timing sample. Keep interrupted attempts and explain them in the audit. If more
samples are needed, predeclare another complete block in a new root and report
both blocks; do not silently replace individual failures or deviations.

Report completion counts before speed. For each workflow show all sample times,
then median setup, GO and combined times among successes. Paired cu/native ratios
use only datasets where both arms passed; below 1 favors cu. Also inspect model
calls, tool calls/errors, returned text, token usage and recorded tool intervals
already collected per turn. These distinguish extra interaction rounds from
slow tool execution. Do not merge workflow medians into a universal speed score.

Setup and GO are measured as full participant turn durations; combined is their
sum. Compilation, app launch, coordinator gaps, offline audit and verification
are outside those times. Text characters exclude images; token usage and tool
call counts are not necessarily equivalent across backends. Non-tool time
includes model, network and scheduling, not just inference. Clean means new
task/app/state, not reboot/VM/cache flush. Keep actual timestamps/order and these
limits in any published report. Publish the aggregate and hashes; keep full local
tool traces private and never export model reasoning.

The protocol's offline regression tests need no GUI or model calls:

```bash
python3 -m unittest discover -s tests/agent -p 'test_benchmark_varied_workflows.py' -v
```

For a separately declared **continuous protocol-v2** experiment, add
`--single-turn` to `prepare`. For the checkbox-state follow-up use a new root,
`--modes triage --repeats 4 --model gpt-6-astra --effort high --single-turn`.
This retains the four paired datasets and alternating order. Create each fresh
task from its generated brief and let it initialize and complete the workflow
in that same turn. Do not pause at READY or send GO. Then collect, stop and audit
the whole turn using the same isolation, backend, exact-outcome and retention
rules. Only the split-setup/no-mutation check is inapplicable.

The report labels this **full task time**, including skill/API loading and
initialization. There is no separately measured setup duration: the legacy JSON
`setup` list is empty and `setup_ms` is 0 because no setup turn exists, not because
initialization was free. `execution_ms` and `combined_turn_ms` both hold the full
turn duration; there is no GO-only ratio. Never pool these timings with protocol
v1 or claim that removing the turn boundary alone caused a cross-block change.

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

## Varied workflows with a fresh task for every attempt

The next comparison (2026-10-31, candidate `c01665d`, production binary unchanged)
uses three different native AppKit workflows, each with two paired datasets:

- **Ticket triage:** filter Open tickets, process six records across pages, apply
  conditional queue/SLA/urgency rules, then search and correct one record.
- **Document editing:** transform six multiline Chinese sections, preserve exact
  punctuation/newlines, mark reviewed, reorder twice, then correct one heading.
- **Configuration wizard:** four profiles with a real route dropdown and
  conditional proxy field, Network/Policy/Review pages, a required return/edit,
  confirmation sheets, and deterministic asynchronous saves (0.35–0.95 seconds).

Every attempt gets a new Codex task, unique app bundle ID/path, fresh process and
saved state. Valid cu attempts also use fresh Broker/output directories and a task-specific
client key on every app call. The task receives only its brief and live UI;
no prewritten solver, source/expected/saved-file access, other task history, or
correctness/timing feedback. Setup reads the skill/API documentation and observes
once without mutations. GO uses that same untouched process. All actual setup
and execution metadata records `gpt-6-astra`, `high`, no service-tier override.
Only one participant runs at a time, including initialization.

“Clean” means these task/app/state boundaries, **not** a reboot, VM, flushed
system caches, or identical backend load. The cases use custom native controls
with AX value setters; they do not establish browser/Electron/commercial-app
performance. Setup is now measured explicitly. Combined time sums setup and GO
turn durations; fixture compilation/launch, coordinator gaps and offline checks
are excluded. The existing tool/token metric limitations above still apply.

| Workflow / dataset | cu setup | Native setup | cu GO | Native GO | cu combined | Native combined |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Triage 1 | 93.5 s | 25.7 s | 207.5 s | 126.0 s | 301.0 s | 151.6 s |
| Triage 2 | 61.4 s | 15.4 s | 162.6 s | 113.5 s | 224.0 s | 128.8 s |
| Document 1 | 68.8 s | 21.3 s | 177.0 s | **failed at 323.3 s** | 245.9 s | **failed at 344.6 s** |
| Document 2 | 50.1 s | 20.4 s | 147.6 s | 197.5 s | 197.7 s | 217.9 s |
| Wizard 1 | 16.1 s | 18.1 s | 121.5 s | 182.4 s | 137.5 s | 200.5 s |
| Wizard 2 | 27.8 s | 26.3 s | 147.2 s | 227.5 s | 175.0 s | 253.8 s |

Independent exact checks passed **6/6 cu** and **5/6 native**. Native document-1
clicked Move S103 instead of Open S103, then closed the main window while trying
to dismiss the move page. Its first two bodies also contained literal `\n` text.
It reported blocked after failed recovery. Native document-2 recovered from eight
recorded AX/tool errors and passed. Failed durations are time to reported blocker,
not time to successful completion, and are not included in a success-time average.

cu was slower on both triage datasets: mean GO 185.0 vs 119.7 seconds (1.55x).
Its recorded tool intervals were only 4.86–5.22 seconds vs native 11.57–12.32;
the observed difference is mainly outside those intervals, not evidence of slow
native click dispatch. On the successful document pair cu used 147.6 vs 197.5
seconds. Both valid wizard pairs favored cu (121.5/147.2 vs 182.4/227.5 seconds).
cu setup sometimes guessed the wrong helper path: triage-1 repaired it during
setup, document-2 during GO. All recovery time is retained. These small samples
show task/strategy dependence, not a universal winner or causal speedup estimate.

Two additional cu attempts are retained as **isolation deviations**, outside the
12 primary samples. The original triage-2-cu omitted `COMPUTER_PILOT_HOME`
throughout (67.7 s setup + 207.5 s GO, exact outcome). The original wizard-2-cu
configured a fresh Broker, but three GO app calls bypassed its configuration
(52.5 s + 411.8 s, exact outcome). That slow wizard attempt tried popup set-value,
stale menu refs, keyboard, OCR and coordinates before AXPress/condition-wait
recovery. It is not evidence for the strictly isolated workflow. Each was
replaced by a new task/app/state with the same brief, with no performance feedback.
Both replacements ran after the paired native trial, so the final order is not
fully counterbalanced. The artifact records all 14 attempts and actual order;
slow/error samples with valid isolation were retained.

[Complete results and evidence hashes](benchmarks/varied-fresh-workflows-2026-10-31.json)
include per-turn settings, timing, tokens, errors, exact checks, failed field
comparisons and both deviations. Local traces contain tools only, no reasoning.
All owned fixture processes and dedicated Brokers were stopped after collection.

To repeat the protocol:

```bash
python3 tests/agent/benchmark_varied_workflows.py /new/experiment prepare
python3 tests/agent/benchmark_varied_workflows.py /new/experiment start triage-1-native
# Create one NEW Codex task from the printed brief; save its ID in LABEL/thread.txt.
# Wait for READY. Check actual model and, for cu, task config/Broker isolation.
# Send GO native-1. (or GO cu-1., GO native-2., GO cu-2. for that arm/dataset).
# Wait for completion, then collect before stopping:
python3 tests/agent/benchmark_varied_workflows.py /new/experiment collect triage-1-native
python3 tests/agent/benchmark_varied_workflows.py /new/experiment stop triage-1-native
```

Repeat for the manifest sequence, never reusing a task. Audit every app call's
backend/environment before accepting a sample; a correct setup alone does not
prove later calls retained isolation. Preserve any excluded attempt and its reason.
The fixture passed complete coordinator smoke runs for all three workflows before
formal trials, and the independent verifier rejected deliberately corrupted saved
fields. These are benchmark additions; the earlier production checks remain
applicable because the measured CLI/skill code did not change during this series.

## Wait handoff and simpler sessions

Follow-up `96c9b7b` fixes an error exposed in both valid wizard runs above:
`wait` returned the new list/ref, but did not publish it to the Broker. The next
click was rejected against the older observation. This was a CLI state-handoff
bug, **not** the participant ignoring a fresh-ref requirement. The final wait
snapshot now becomes the latest observation on success and timeout; timeout
output remains full and nonzero. Private protocol 6 retires Brokers that would
discard timeout observations.

The skill helper now supplies one `cu` function for UI reads, waits and actions;
`cu_do`/`cu_see` were replaced, with no compatibility shim or new transport.
The skill locates the helper relative to its own directory and guides dropdown
selection through existing AX actions. It uses the wait result directly instead
of requesting another snapshot. Stale refs require a fresh observation, not a
series of unrelated keyboard/OCR/coordinate attempts.

Successful `perform` no longer queries or returns `available_actions` in either
output mode. Those diagnostics are collected only after a rejected action.
Compact receipts omit echoed app/ref/coordinates and text/key/action arguments,
plus routine key/perform dispatch reminders. Actual field values, method routes,
effect/focus results, degraded-state advisories and errors remain available.
Ordinary JSON retains echoed arguments; it intentionally loses the redundant
success-only `perform.available_actions` field.

Two new isolated `gpt-6-astra / high` tasks used the same briefs and dataset 1 as
above. Both resolved the helper path and retained the task environment on every
app call; both exact outcomes passed. All setup/GO time is retained:

| Workflow | Earlier GO | Follow-up GO | Follow-up setup | Follow-up combined | Returned text, earlier → follow-up |
| --- | ---: | ---: | ---: | ---: | ---: |
| Triage | 207.5 s | 179.6 s | 44.2 s | 223.9 s | 28,426 → 25,993 characters |
| Wizard | 121.5 s | 153.0 s | 21.3 s | 174.3 s | 37,559 → 31,057 characters |

Triage had no tool errors. Wizard used wait refs directly, with no wait-handoff
error; one transient menu identity change was recovered through AX without OCR
or coordinates. Returned text was lower in both samples, but elapsed time moved
in opposite directions. These are forward behavior tests with historical timing
references, **not** a controlled timing estimate or a new native comparison.
[Follow-up records](benchmarks/session-followup-2026-10-31.json) retain settings,
usage, checks, prior samples and tool-only evidence hashes.

Validation: 43 Rust tests, Clippy, release build, version/skill validation, and
45 command-suite checks passed; two TextEdit cases were skipped to preserve the
user's open session. The owned native regression covers menu callback effects,
error action hints, async view replacement followed by direct wait-ref use,
actionable timeout snapshots, exact Unicode contents after receipt compaction,
ordinary JSON, and sourced sessions across sh/bash/zsh. Broker upgrade tests cover
retirement of incompatible versions. No per-field effect checks, automatic retry
framework or new configuration layer was added.

## Standardized triage block (protocol v1)

The first complete protocol-v1 block measured candidate `b2a4cfa` on 2026-10-31.
All eight predeclared attempts ran in the manifest order, sequentially, using
fresh tasks/apps/state and actual `gpt-6-astra / high`, no service-tier override.
Every cu app call retained its isolated task configuration. All eight protocol
and backend audits passed; there were no replacements or excluded attempts.

**Exact outcomes: cu 3/4, built-in native 4/4.** cu dataset 4 reported DONE with
zero tool errors but left Urgent enabled on T403 and T407 (both Normal severity).
The independent verifier rejected both original and final records. Its short
69.6-second duration is a failed outcome, not a successful speed result.

| Dataset | Order | cu setup / GO / combined | Native setup / GO / combined | Exact outcome cu / native |
| --- | --- | ---: | ---: | --- |
| 1 | native → cu | 49.6 / 137.9 / 187.5 s | 23.9 / 183.7 / 207.6 s | pass / pass |
| 2 | cu → native | 48.9 / 137.5 / 186.4 s | 23.0 / 166.0 / 189.0 s | pass / pass |
| 3 | native → cu | 19.4 / 74.0 / 93.5 s | 14.6 / 102.5 / 117.1 s | pass / pass |
| 4 | cu → native | 20.5 / **failed 69.6** / 90.1 s | 24.2 / 149.0 / 173.2 s | **fail** / pass |

For the three jointly successful pairs, median paired cu/native ratios were
**0.751 for GO** and **0.903 combined** (about 24.9% and 9.7% lower cu time).
These conditional timing results do not offset the lower observed completion
rate or include repair cost. The arm-wise success medians use different subsets
(cu three, native four), so they should not replace the paired comparison.
No causal speedup from the code change can be inferred by comparing this block
with historical timings.

Recorded GO model calls were cu `20 / 20 / 21 / 21` and native
`34 / 23 / 22 / 22`. Recorded tool intervals were cu
`5.14 / 5.12 / 4.87 / 4.43 s` and native `17.40 / 11.89 / 11.79 / 11.53 s`.
The large remaining time includes model, network and scheduling. Similar call
counts had very different durations, and cu setup used the same four shell
calls while ranging from 19.4 to 49.6 seconds. Backend latency and strategy
variation remain material even with matching model metadata. cu also returned
more text than native in datasets 2–4; compact mode does not imply universally
smaller observations across implementations.

Two concrete findings guide the next work:

- **Checkbox state is missing from cu's element value.** For T403 and T407,
  cu returned a checkbox with title Urgent and no value; native returned
  `checkbox Urgent, Value: 1`. cu did include adjacent `Urgent status: on` text,
  so the evidence does not prove omission of the element value alone caused
  the agent mistake. Both snapshot readers in `src/ax.rs` accept only string
  AXValue; numeric checkbox state is lost before compact formatting. The next
  targeted correction is to preserve scalar AXValue using the existing read,
  with an on/off behavior regression. This requires no additional effect check
  or per-action snapshot. It is not fixed or retested by this benchmark block.
- **The READY/GO boundary adds native recovery overhead.** Every native first
  GO action received “Computer Use is not active” and recovered with
  `getAXState()`. All four errors and full recovery times remain included.
  A future separately declared single-turn experiment should measure ordinary
  uninterrupted task execution; do not silently subtract or exclude these
  calls from protocol-v1 results, or generalize its ratios to that workflow.

[Complete standardized records](benchmarks/triage-standardized-2026-10-31.json)
retain every sample, actual order/settings, setup and GO usage/timing, independent
checks, failure field differences, manual audit notes, evidence hashes and limits.
Full tool-only traces remain local. The generated local JSON/Markdown report
includes pending/unreviewed/excluded statuses when applicable; none occurred here.
All owned apps and dedicated Brokers were stopped. No production code or benchmark
inputs changed during the block. This update publishes evidence only, so the
existing code/CI checks remain applicable; JSON and diff checks passed.
