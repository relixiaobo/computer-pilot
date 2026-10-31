# Agent End-to-End Tests

Real LLM agent tests for `cu`. Unlike `tests/commands/` (which test "does the command work"),
these tests verify "can an agent complete a real task correctly".

## Real application comparisons

`benchmark_real_apps.py` reuses the rollout collector and status/pair reporting
from the varied-workflow benchmark, with real applications and saved-artifact
verification. This is an application-efficiency pilot, not an AX microbenchmark.

```bash
python3 tests/agent/benchmark_real_apps.py test-results/real-apps-NEW prepare
python3 tests/agent/benchmark_real_apps.py test-results/real-apps-NEW start LABEL
# Run the generated brief in a fresh model profile and neutral cwd, save thread.txt.
python3 tests/agent/benchmark_real_apps.py test-results/real-apps-NEW collect LABEL --rollouts PROFILE/sessions
python3 tests/agent/benchmark_real_apps.py test-results/real-apps-NEW stop LABEL
# Audit each complete trace/context, then:
python3 tests/agent/benchmark_real_apps.py test-results/real-apps-NEW report
```

The manifest freezes six attempts, one matching dataset per application, with
alternating arm order, gpt-6-astra/high, no tier override, application versions
and binary/skill/coordinator hashes. The same fresh-profile procedure used below
removes global/project AGENTS and previous model history. Each full task turn
includes skill/API discovery and setup. App launch, coordinator checks and cleanup
are outside the timer; the runner enforces the predeclared 600-second wall limit.
All failures, interruptions, protocol deviations and recovery costs are retained;
no individual attempt is replaced.

- TextEdit: revise six Chinese sections, preserving unrelated text and newlines,
  then save the original plain-text file. Verification reads disk bytes as text.
- VS Code: inspect and update a JSON service config, Markdown release note and
  text checklist; save all three. Verification checks every requested field and
  unchanged content, rather than the visible editor buffer alone.
- Numbers: update two inputs, calculate six row formulas and a total, sort the
  data rows, rename the sheet and save a native document. Verification closes the
  task document without saving, reopens its saved file, and independently reads
  cell values, formulas, order and dimensions through application scripting.

The user explicitly chose shared TextEdit/VS Code processes with dedicated test
windows and independent file copies. Existing settings, extensions and caches
therefore remain shared; this must not be described as complete app isolation.
Numbers starts separately. Shared apps are not quit during cleanup; only the
observed matching task window is closed. Participants must not change user settings
or act on other windows, and may foreground their test window when input needs it.

cu may use either its UI commands or app-native document scripting through
`cu tell`, following the packaged skill. Native uses the actual Computer Use App
API. Direct document filesystem edits, integrated terminal/script execution,
external backends and prepared solvers are forbidden. Report the route used:
a scripting win is product-level task efficiency, not faster AX clicking.
Only compliant successful pairs receive a timing ratio. A correct file produced
after changing shared user settings is still a protocol deviation.

### Real-app pilot — 2026-10-31

[Audited results](benchmarks/real-apps-2026-10-31.json) retain all six attempts
against frozen candidate `75967e0`. TextEdit 1.20, VS Code 1.142.0 and Numbers
15.1 ran with gpt-6-astra/high and Codex CLI 0.165.0. Actual initial contexts
confirmed six distinct profiles/cwds with no AGENTS injection. Production code,
skill and helper were unchanged throughout; only this report was added afterward.

| Application | Built-in Computer Use | cu | Tool calls / errors: native; cu |
| --- | --- | --- | --- |
| TextEdit | **Passed, 53.219 s** | Failed, 88.689 s: visible text correct, saved file original | 3 / 0; 6 / 1 |
| VS Code | Excluded, 79.638 s: correct files, changed user setting | Failed, 110.599 s: Select All ineffective, insertion stopped unsaved | 16 / 1; 15 / 3 |
| Numbers | Interrupted at 599.748 s by 600 s wall cap; no saved output | **Passed, 125.107 s**, app-native scripting | 39 / 2; 14 / 6 |

There are two accepted successes, two completed failures, one settings deviation
and one timeout. **No application has two compliant successes, so there is no
paired speed ratio or overall speed ranking.** Failed/excluded durations describe
time spent, not successful completion speed. Numbers native counts cover completed
calls before interruption; its zero complete-turn accumulator is not a duration.

The traces identify follow-up work without establishing every root cause:

- TextEdit cu grouped an AX value write, PID-targeted Save and final snapshot.
  The snapshot looked correct while disk content was untouched. Diagnose document
  edit notification and shortcut delivery before changing generic verification;
  the observation alone cannot distinguish these causes.
- VS Code cu's Select All left the old content in place. Verified typing detected
  an unknown outcome and the agent stopped. Native completed the files after
  enabling screen-reader accessibility with `shift+alt+F1`, which changed the
  shared user setting `editor.accessibilitySupport`. That violates this block's
  rules. Only the added setting was removed, restoring bytes identical to the
  pre-task local-history baseline. Future editor trials need an explicit common
  accessibility baseline before timing; this attempt is not replaced.
- The helper's argument contract caused duplicate `--no-snapshot` and missing
  application arguments for commands outside its targeted-action cases. Clarify
  that general contract before adding per-application recovery instructions.
- Numbers cu recovered from six discovery/script errors, including a sort error
  after two writes had already applied. It read current values before continuing.
  Native spent repeated rounds on cell selection/input and never completed sort
  and save. This supports testing script discovery and input semantics separately;
  it does not establish that cu's AX operations are faster.

Tool interval totals were 2.191/0.181 s for TextEdit native/cu, 13.344/2.774 s
for VS Code, and 38.573/3.092 s for Numbers (native partial). The remainder includes
model, network and orchestration time, not just model reasoning. Fewer recovery
rounds and correct command discovery may matter more than shaving dispatch time.

This is one dataset per application, with shared TextEdit/VS Code settings,
extensions and caches. LibreOffice failed startup during untimed preflight and
was replaced with Numbers before freezing the block; it is not a timed failure.
All owned app/Broker processes stopped and the shared user apps remained alive.
Cleanup needed an explicit Don't Save on the failed VS Code task window after
automatic close left an unsaved-changes dialog. Both shared apps were then checked
for remaining test windows. Future runs must inspect such dialogs rather than
treat a close dispatch or cleared coordinator PID as proof of cleanup.

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

## Current snapshot contract

Snapshots now have one state contract: complete elements within the requested
limit, with full labels/values. This also applies to action and wait results.
`--compact` only selects concise presentation; ordinary JSON retains paths,
geometry and display metadata. Automatic deltas, their baseline checks and tree
copies, `snapshot --diff` and its disk cache, and action-text truncation have
been removed. `src/diff.rs` now only counts changes in memory for action-effect
verification. Ref/client/PID/window identity checks and explicit verification
options remain; they prevent wrong-target actions and detect dropped input.

Broker and AX share one element type. Window identity is read from the same AX
window that supplies the tree and reused for publication/freshness checks;
there is no second window geometry query. Snapshots carry their PID even when
an application has no window. Private Broker protocol 8 retires old response
semantics. Skill instructions, CLI help, recipes and command tests match this
contract. The general three-workflow block below measures this snapshot cleanup;
earlier blocks describe their own frozen historical commits.

Validation: 43 Rust tests, 8 offline protocol tests, Clippy with warnings denied,
formatting/version checks and release build passed. Eleven affected command
suites passed 138 checks, with 4 TextEdit-dependent checks skipped to preserve an
existing user session. The owned AppKit fixture covers complete action/wait
state, exact long Unicode values, unchanged actionable refs, checkbox callbacks,
client isolation and rejection of an old ref after switching to a window with
identical title/field/value. Screenshot/state tests passed with a temporary
Finder window and a canonical `/private/tmp` output path. Initial runs without a
Finder window or through the `/tmp` symlink failed their environment assumptions;
only affected suites were rerun after correcting those conditions. The removed
`--diff` flag is rejected, and snapshots no longer create a disk cache.

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

For a cu-only retest, add `--arms cu`. Only those attempts are planned and
reported; no native task runs or paired ratio is fabricated. Freeze the historical
report references before running, match workflow/dataset/settings, and show all
selected historical rounds separately as noncontemporaneous controls.

For **each** manifest entry, sequentially:

1. Run `ROOT start LABEL`. Create one new task from the exact generated brief,
   using the planned settings, and save its ID in `ROOT/LABEL/thread.txt`.
   Start participant tasks in a neutral working directory outside this repository,
   without the project's `AGENTS.md`/`CLAUDE.md` in their injected instructions.
   Those files guide development; the skill (cu) or native API documentation
   supplies computer-use instructions. The brief uses an absolute skill path.
   Inspect actual initial context as part of the audit: a new task alone does
   not prove a clean runtime context. Remove **global** AGENTS instructions too;
   changing cwd or setting `project_doc_max_bytes=0` alone does not do this.
   If the host only inherits the coordinator's
   repository directory, report a repository-context diagnostic, not a clean
   skill-only performance comparison.
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

   Also review initial injected instructions for repository development guidance.
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

### Clean Codex runtime context

Use a fresh `CODEX_HOME` and a neutral cwd for **each participant**, with no
`AGENTS.md`, `AGENTS.override.md`, session/history databases or memories copied
into that profile. Keep authentication and the same installed skills/runtime
available. Copy configuration privately; never rename the user's global AGENTS
file, change their global configuration, or ask the participant to ignore
instructions that were already injected.

With Codex CLI 0.165.0, the bundled marketplace must be copied into the new
profile's `.tmp/bundled-marketplaces/openai-bundled`, and its configured source
must point there. Merely pointing the new profile at the original reserved
marketplace left native Computer Use unavailable. Copy its installed plugin
cache as well. Verify the actual `mcp__cua_repl.js` capability in a separate
non-task probe before the measured block; do not substitute another backend.

Launch each generated brief with the same CLI binary and settings, for example:

```bash
CODEX_HOME="$BENCH_PROFILE" codex --no-daemon exec \
  -C "$BENCH_WORKDIR" --skip-git-repo-check \
  -m gpt-6-astra -c 'model_reasoning_effort="high"' \
  -c project_doc_max_bytes=0 -c features.memories=false \
  -c 'approval_policy="never"' -s danger-full-access \
  --json --color never -o "$TRIAL/final.txt" - \
  < "$TRIAL/brief.txt" > "$TRIAL/events.jsonl" 2> "$TRIAL/stderr.log"
```

Unset inherited `CODEX_SESSION_ID`, `CODEX_THREAD_ID` and `CODEX_TOOL_CALL_ID`
in the participant process environment. Save its launch arguments, thread ID,
and initial-context audit alongside the normal evidence. Collect with
`--rollouts "$BENCH_PROFILE/sessions"`. Inspect the actual initial user and
developer messages for global/project AGENTS content and the `turn_context`
for cwd/model/effort/tier; absence of files alone is insufficient. Hash task
shell configuration in its actual location, including when it lives in the
neutral cwd rather than the fixture directory.

This isolates runtime instructions, history and task/app state. It does not
reset macOS, Launch Services, user permissions or system caches. Keep exact
app/PID bindings and disclose any recovery that resolves a generic name to an
old fixture. Retain and exclude that attempt; do not replace it with a clean
success. An interrupted turn's zero completed-turn sum is **not** its elapsed
time: report the interruption duration separately and omit it from successful
timing comparisons.

## Task setup and known transitions follow-up (2026-10-31)

The six-attempt retest exposed two distinct costs: repeated observations of known
asynchronous transitions, and a generated config that omitted its helper. Candidate
`0a688a6` addresses these in the packaged skill, without changing the Rust binary:

- Copy the existing helper into the task config and append the supplied values.
  Every later shell sources that one file, which contains both target binding
  and stop-on-error. No generated wrapper, extra startup command or validation.
- End a known asynchronous action group with `wait`, which already returns the
  next snapshot. Observe an unfamiliar flow first, then reuse it on later items.
- Use the existing default wait timeout; custom `wait --timeout` values are seconds.
  No invented milliseconds flag or compatibility alias.

The owned command fixture now uses the copied config. Independent sh/bash/zsh
processes run with a competing `cu` on PATH and without a shell `-e` option:
the configured binary edits the exact fixture, and a rejected action stops later
edits instead of being hidden by a successful final snapshot. The suite, skill
validation and hosted CI passed. The helper's runtime logic is unchanged.

Two cu-only wizard tasks were predeclared in fresh profiles, neutral workspaces,
independent fixture states and private runtimes. Actual contexts contained no
global/project AGENTS; model/settings match earlier blocks. Both passed every
independent original/final/correction check. Native was not rerun, and no attempt
was replaced or coached. All observed owned app/Broker PIDs stopped.

| Dataset | Full task s | Tool calls | Tool errors | Tool interval s | Output tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 157.040 | 31 | 0 | 10.010 | 3,343 |
| 2 | 275.523 | 31 | 0 | 11.220 | 3,395 |

Both copied the helper without reading its source, kept the supplied binding on
every call, and used default wait timeouts. Dataset 1 observed the first unfamiliar
saving state, then combined each of the next three confirmations with its wait.
This removed the three repeated round trips present in the preceding accepted
wizard-1 trace (34 calls, 180.765 s). Dataset 2 combined all four confirmations and
waits. Four required menu selections exposed their updated controls without stale
errors. One unnecessary opening of an already-correct dropdown remains in dataset
2; the skill already says to leave correct values alone, so no duplicate rule or
task-specific exception was added for that deviation.

End-to-end speed is **not consistently improved**. Dataset 1 matches the latest
historical native time (156.766 s), but dataset 2 exceeds the latest valid cu/native
times (182.350/126.096 s). The immediately preceding 240.218-second cu dataset 2
remains excluded for reading the wrong app and is not a valid speed baseline.
Earlier rounds and their hashes are retained in the
[follow-up artifact](benchmarks/cu-flow-2026-10-31.json).

The two new tasks differ by 118.483 s despite identical call counts and almost
equal output tokens; 117.273 s of that difference is outside tools. Median gaps
between tools are 4.260 versus 7.902 s (maxima 7.868/12.710 s), so the slower run
has broadly slower rounds, not one large UI stall. The rollout cannot separate
inference, provider queues, transport and host scheduling. Historical cu snapshots
also generally returned less text than native, so these traces do not support
blaming verbose cu observations for the slowdown.

The practical next performance measurement is request/first-token/completion
timing at the model host, if that host exposes it, or another predeclared matched
block when native reruns are authorized. Neither belongs in the public cu CLI.
Do not add caches, fixed sleeps, more automatic inspections or remove target/ref
identity checks to compensate for unexplained time outside tools. The supported
improvement here is fewer repeated decisions and successful initialization in
both trials, not a universal latency win.

## cu-only retest with historical controls (2026-10-31)

Predeclared six cu attempts (three workflows, two datasets); **5 accepted passes,
1 excluded protocol deviation, no replacements**. All six reached independently
verified final states, but final correctness does not repair a broken target
boundary. Native Computer Use was **not rerun**. The four historical clean reports
and their hashes were fixed before execution and are embedded in the
[retest artifact](benchmarks/cu-retest-2026-10-31.json).

Candidate `5a06abc` changes only the benchmark coordinator to allow `--arms cu`.
The binary, skill, helper and fixture match `7954532` byte-for-byte. Each run used
gpt-6-astra/high, Codex CLI 0.165.0, no service-tier override, a fresh profile and
neutral cwd, independent exact fixture PID/state and private cu runtime. Actual
initial contexts contained no global/project AGENTS. Skill loading and setup
are inside the single full-task timer. No coaching or mid-block changes occurred.

| Current attempt | Full task s | Calls | Tool errors¹ | Tool interval s | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| triage-1-cu | 147.740 | 21 | 0 | 4.960 | passed |
| triage-2-cu | 129.153 | 21 | 0 | 5.013 | passed |
| document-1-cu | 115.432 | 22 | 0 | 5.310 | passed |
| document-2-cu | 124.774 | 22 | 0 | 5.104 | passed |
| wizard-1-cu | 180.765 | 34 | 0 | 7.168 | passed |
| wizard-2-cu | 240.218 | 34 | 1 | 11.281 | excluded |

¹ Tool errors count failed host calls. In wizard-2, a rejected untargeted
`click` was followed by a successful snapshot in the same shell; the shell exited
zero, so that CLI error is additionally recorded by manual audit, not counted as
a failed host call.

All historical sample times below are retained separately. Columns 1/2 are the
same source datasets as the current block; units are full-task seconds. These
are **historical controls, not contemporaneous pairs or causal estimates**.

| Workflow | Block | cu 1 / 2 | Native 1 / 2 |
| --- | --- | ---: | ---: |
| triage | skill-only | 116.107 / 80.595 | 96.880 / 80.521 |
| triage | decision-boundaries | 91.110 / 125.998 | 98.705 / 99.804 |
| triage | cu-retest | 147.740 / 129.153 | — / — |
| document | skill-only | 83.707 / 157.993 | 100.031 / excluded (interrupted 221.690) |
| document | decision-boundaries | 128.470 / 127.186 | 144.586 / 143.389 |
| document | cu-retest | 115.432 / 124.774 | — / — |
| wizard | skill-only | 298.765 / 338.202 | 179.675 / 150.838 |
| wizard | decision-boundaries | 190.958 / 138.150 | 165.112 / 221.683 |
| wizard | recovery-state | 238.292 / 198.001 | 163.614 / 175.730 |
| wizard | menu-transition | 104.485 / 182.350 | 156.766 / 126.096 |
| wizard | cu-retest | 180.765 / 240.218 (excluded) | — / — |

Triage medians were current cu **138.4 s**, prior decision-boundaries cu **108.6 s**
and native **99.3 s**. Current calls were 21/21 versus prior cu 21/22; current tool
intervals were about 5 s each, slightly lower than before. The slowdown therefore
occurred outside tools. Document median was **120.1 s**, versus prior cu **127.8 s**
and native **144.0 s**, still with 22 calls per task. Older native document-1 was
100.0 s; this is not consistent dominance across all historical rounds.

Only wizard dataset 1 supplies an accepted new timing: **180.8 s**, versus the
same-binary previous cu **104.5 s** and historical native **156.8 s**. Current cu
used 34 calls, versus 31/32 previously. Four saves produced a saving-state
snapshot followed by a separate wait; three repeated an already observed flow.
Five `--timeout 10000` values mean seconds, not milliseconds; all conditions met
promptly. Both menu selections returned the new controls directly with no stale
menu errors. No task-specific fix or compatibility flag was added during testing.

Wizard-2 completed in **240.2 s** but is excluded. Its generated task config
exported the supplied variables without sourcing `session.sh`. The next shell
sourced only that config: bare `cu` resolved through PATH, the untargeted click
was rejected with `observation_required`, and a following snapshot read a
non-fixture application. The model restored the helper and target for subsequent
calls and completed the fixture. It also tried a nonexistent helper path once.
The missing helper source bypassed both target binding and shell stop-on-error;
this is a concrete setup failure, not a reason to remove observation identity
checks. Full unrelated UI output stays local and is not exported. Its correct
final state and two successful menu transitions are diagnostic evidence only,
not an accepted performance sample.

Tools occupy roughly **3–5%** of accepted elapsed time. The remaining interval
includes model inference, service/host scheduling and other inter-tool overhead;
it cannot be identified as model thinking or a specific provider delay from
these traces alone. Repeated same-binary variability and historical controls
limit speed claims. Remaining concrete costs are repeated async observations,
ambiguous timeout use and fragile generated helper initialization.

All planned traces/configs/context records were reviewed and fingerprinted.
All observed benchmark app/Broker PIDs stopped; no stopped runtime was restarted
for verification. System permissions, caches and native service assets remain
shared; this is not VM isolation. The coordinator change passed 10 offline
protocol tests and hosted CI; unchanged Rust code was not rebuilt or retested.

## Recovery state and menu transitions (2026-10-31)

Two further frozen four-attempt wizard blocks tested the remaining recovery and
menu costs. All eight tasks passed, using the same model/settings and protocol-v2
fresh-profile procedure below. Actual initial contexts contained no global or
project AGENTS. Each attempt had its own neutral cwd, app state and, for cu,
Broker/client/output directory. There were no replacements, coaching or input
changes within either block.

The first candidate, `7ee446b`, fixes discarded recovery state: a stale-ref check
already reads the current UI, so its error now returns that same snapshot in
`diagnostics.snapshot` and registers the refs. The Broker captures published
observations from either output stream without a success/wait special case.
Compact error snapshots use the same presentation as successful snapshots while
preserving error metadata and full text. No extra AX read or validation is added.

| Candidate / dataset | cu seconds | Native seconds | cu / native calls | cu / native errors |
| --- | ---: | ---: | ---: | ---: |
| Recovery state / 1 | 238.292 | 163.614 | 37 / 34 | 2 / 0 |
| Recovery state / 2 | 198.001 | 175.730 | 32 / 33 | 2 / 1 |
| Menu transition / 1 | 104.485 | 156.766 | 31 / 32 | 0 / 0 |
| Menu transition / 2 | 182.350 | 126.096 | 33 / 33 | 1 / 0 |

In the recovery-state block, cu directly reused three of four stale-error
snapshots; one still prompted a redundant observation. This removed real recovery
work but did not establish an overall speed improvement: the median paired
cu/native time ratio was **1.292**. A premature menu selection plus navigation
also exposed a race: the UI reached the policy page, then a delayed menu callback
rebuilt the network page. Native also repeated an accepted menu selection and
recovered from an invalid element ID in its second attempt.

Separate owned AppKit probes isolated that race. Menu-item AXPress returned in
**73–81 ms**, while the immediately following snapshot still contained the menu.
`AXMenuClosed` arrived roughly 400 ms after the probe was armed; by then the new
selection and dynamically added/removed fields were visible. The final candidate,
`7954532`, reuses the existing AX observer and 500 ms deadline specifically for
menu-item AXPress. It subscribes before dispatch to menu close/open notifications
(the latter allows submenu opening), then returns after a notification or the
deadline. It adds no snapshot, polling loop, retry or fixed sleep. Missing
subscription support keeps the existing immediate-dispatch behavior; this is a
bounded transition wait, not a universal proof that an app finished all work.

After this change, ref perform, ref click and path perform took **447–489 ms** in
the dynamic-menu probe. Their immediate snapshots all showed the correct selected
value and conditional fields, with no departing menu. The new menu subscription
does not apply to ordinary field actions. The skill also observes controls after
menu selection and uses `./cu-env.sh` in a fixed task cwd, retaining absolute paths
when cwd changes.
In the preceding cu traces, repeated source paths accounted for 2,380/2,040 of
4,088/3,595 subsequent command characters; short paths remove that repetition.

In the final block, all four cu menu selections led directly to the expected
controls: **zero stale menu errors**, compared with four stale errors in the
preceding block. Output tokens fell from **5,696/4,858 to 3,489/3,755** (31% fewer
combined); calls were **31/33**, versus native's **32/33**. Median full-task times
were **143.4/141.4 seconds** for cu/native, and the median paired ratio was
**1.056**. Each arm won one pair. Tool intervals were cu **9.122/8.976 seconds**,
native **19.583/19.495**; most elapsed time remains outside tools. These small
historical blocks do not isolate the timing effect of each change or prove a
general speed advantage.

Remaining strategy costs are retained. cu still sometimes took a snapshot before
a known wait; its second final attempt read the helper and invented
`--timeout-ms`, then recovered without repeating the preceding confirmation.
Later `--timeout 10000` values were seconds, not milliseconds, although all those
conditions met promptly. No compatibility alias or task-specific solver was
added. Native used a screenshot and extra state reads around one menu transition.

Validation: 43 Rust tests, Clippy, release build and skill validation passed.
The owned command fixture verifies recovery after a changed value, missing ref
and changed window without a redundant snapshot; stale writes remain rejected.
It also verifies immediate menu callback state through all three action entry
points. One intermediate run hit the existing long-Unicode snapshot-equality
assertion; two subsequent isolated runs passed, with typing code unchanged.
Benchmark/probe-owned processes were stopped, and actual configuration/plugin
hashes and non-overlapping execution were checked. macOS user-session state and
the native service remain shared, so this is not a VM-isolation result.

[Recovery-state attempts](benchmarks/recovery-state-2026-10-31.json) and
[final menu-transition attempts plus probe measurements](benchmarks/menu-transition-2026-10-31.json)
retain every outcome, audit and evidence hash. Full traces and reasoning stay local.

## Shorter skill follow-up (2026-10-31)

Candidate `bce1315` shortened the skill entrypoint from 289 lines / 13,744 bytes
to 152 lines / 7,824 bytes. Everyday setup and grouped edits come first; the
controlled-editor verification path stays in the recovery reference. The skill
now explicitly groups known edits, navigation and the final observation in one
shell call, skips already-correct values, resumes grouping after recovery, and
explains departing menus and destination-specific wait conditions. The command
reference also clarifies that ordinary actions use the latest observation;
explicit observation IDs select an earlier one. These are general instructions,
not fixture-specific solutions. No CLI code/binary, helper, fixed delay or
runtime validation changed from the preceding clean block.

All **12/12 attempts passed** in a new protocol-v2 block: fresh Codex profiles,
neutral working directories and disposable app state, with no global/project
AGENTS in any actual initial context. Model/settings, CLI version, native plugin,
fixture and alternating order match the previous clean block. Inputs stayed
frozen; no coaching, interruptions or replacement attempts occurred. Skill/API
initialization remains inside the measured full task turn.

| Workflow / dataset | cu full task s | Native full task s | cu / native tool calls |
| --- | ---: | ---: | ---: |
| Triage 1 | 91.110 | 98.705 | 21 / 20 |
| Triage 2 | 125.998 | 99.804 | 22 / 21 |
| Document 1 | 128.470 | 144.586 | 22 / 29 |
| Document 2 | 127.186 | 143.389 | 22 / 29 |
| Wizard 1 | 190.958 | 165.112 | 33 / 33 |
| Wizard 2 | 138.150 | 221.683 | 40 / 34 |

Median paired cu/native time ratios are **1.093 / 0.888 / 0.890** for triage,
document and wizard, respectively. Wizard cu calls fell from the previous
block's **44/62 to 33/40**; neither attempt degraded into per-field snapshots
after recovery. Document calls were consistently 22, compared with 22/33 before.
Triage remained 21/22. This supports the narrower finding that grouping was more
consistent; two samples per workflow and a historical comparison do not prove
a causal or universal speed gain. For example, the 40-call cu wizard finished
faster than its 33-call counterpart. Native wizard timings also moved from
179.675/150.838 to 165.112/221.683 seconds with an unchanged runtime.

Remaining costs are retained in the results:

- cu wizard 1 repeated an accepted menu selection and received a stale-ref error.
  It also waited past an unhandled confirmation sheet, consuming 10.162 seconds,
  then redundantly observed the state already attached to that timeout.
- cu wizard 2 reselected two already-correct dropdown values and repeated two
  accepted menu selections. Identity checks rejected the stale refs before
  dispatch. Its profile-ID waits were not necessarily unique to the destination.
  Shorter instructions did not reliably eliminate these strategy mistakes.
- cu document 1 guessed nonexistent `observe` while loading the skill; the
  parser rejected it before app access. Native documents each recovered from
  seven AX errors. Native wizard 2 took a screenshot and recovered from a
  non-opening label click. All these costs remain inside the timers.
- Wizard tool intervals were cu **20.936/10.341 seconds**, native
  **19.114/20.238**. Time outside tool intervals still dominates and includes
  model/network/scheduling time; it is not a measurement of pure reasoning time.

Separate small AppKit probes reproduced a menu remaining visible immediately
after accepted selection. They did not establish a blanket menu-expiry problem:
items remained actionable after 0/1/6-second gaps before selection. This supports
observing transitions, not adding blind sleeps or removing identity checks.

All profile config hashes and bundled/native-cache plugin fingerprints were
checked, participant turns did not overlap, and all owned app/Broker processes
were stopped. The native service/runtime assets and macOS user session,
permissions, Launch Services and caches remain shared; this is not VM isolation.
Skill metadata validation and changed-reference link checks passed. Existing
code/command validation remains applicable because runtime code is unchanged.

[All 12 outcomes, actual-context audits, timings and evidence hashes](benchmarks/decision-boundaries-2026-10-31.json).
Full local traces and model reasoning are not published.

## Skill-only runtime block (2026-10-31)

The post-fix candidate `4b611c6` was tested in 12 fresh CLI sessions with **no
global or project AGENTS instructions injected**. cu passed 6/6 attempts; native
passed 5/6, with one incomplete protocol deviation retained and excluded.
Removing development instructions did **not** eliminate the wizard timing gap.

Each participant used a new private Codex profile, neutral cwd, fixture process,
state and, for cu, Broker/client/output directory. Actual initial messages were
audited, not inferred from the launch command. All sessions used Codex CLI
0.165.0, `gpt-6-astra` / `high`, no service-tier override, and native
`unified-computer-use` 26.1027.30824. They ran sequentially in the predeclared
alternating order. No model coaching, replacement attempts or runtime/skill
changes occurred during the block. Protocol v2 includes skill/API initialization
in the same measured task turn; host provisioning/startup/shutdown is outside it.

| Workflow / dataset | cu full task s | Native full task s | Outcome |
| --- | ---: | ---: | --- |
| Triage 1 | 116.107 | 96.880 | Both passed |
| Triage 2 | 80.595 | 80.521 | Both passed |
| Document 1 | 83.707 | 100.031 | Both passed |
| Document 2 | 157.993 | — | cu passed; native interrupted after 221.690 s and excluded |
| Wizard 1 | 298.765 | 179.675 | Both passed |
| Wizard 2 | 338.202 | 150.838 | Both passed |

Success medians (cu/native seconds) are triage **98.4/88.7**, document
**120.8/100.0**, and wizard **318.5/165.3**. Document medians use unequal
success counts and are not a paired estimate. Median paired cu/native ratios
are **1.100** for triage (two pairs), **0.837** for document (one pair), and
**1.952** for wizard (two pairs). These small samples do not establish a general
speed ranking, nor a causal before/after effect of removing AGENTS; keep older
repository-context experiments separate.

The traces explain the remaining costs:

- cu triage used 21/22 tool calls versus native's 20/20, with no errors. The
  leading `--compact` path appeared naturally and kept the correct target.
- cu document 1 tried nonexistent `observe` before using the skill's snapshot
  path. Document 2 used 33 calls versus the first one's 22, splitting many
  mutations and boundary snapshots across calls. An old ref after Save was
  rejected before dispatch, then recovered with fresh state. Both exact outcomes
  passed; these errors remain in the timing.
- Native document 1 recovered from seven `AXError.failure` responses and passed.
  Document 2 saved all originals, reopened a section instead of moving it, then
  clicked the labeled window close button. Recovery resolved a generic app name
  to an old fixture identity. The coordinator interrupted that deviation; order,
  final correction and completion were missing. Its 35 calls, 16 errors and
  221.690-second interruption remain recorded; it is not a zero-time completion.
- cu wizard used **44/62 calls**, versus native's **32/31**. Menu transitions
  invalidated refs, broad wait text could already match navigation labels, and
  one wait for the list ran while a confirmation sheet was pending (10.149-second
  timeout). The second cu task adopted per-field snapshots despite the skill's
  grouping guidance. Repeated reads and the same-target OCR detour are retained
  strategy costs, not exclusions.
- Wizard tool intervals totaled **21.595/9.726 seconds** for cu and
  **20.140/20.187 seconds** for native. Most of the total gap lies outside those
  intervals: extra model rounds, model/network/scheduling time and recovery.
  This does not justify removing target identity checks that prevented wrong
  dispatch, or adding task-specific solvers and aliases.

The macOS user session, permissions, caches and Launch Services were shared;
this was not a VM reset. In particular, old fixture names remained registered,
which the excluded native recovery reached. Exact app binding and actual-context
audits are therefore part of the result, not a claim of complete OS isolation.
All task-owned app processes and Brokers were stopped; unrelated sessions were
preserved. After the block, only offline reporting was adjusted to retain
interrupted duration/order and avoid rendering absent completion time as zero.

[All 12 outcomes, runtime audits, timings and evidence hashes](benchmarks/skill-only-2026-10-31.json).
Full tool traces and model reasoning are not published.

## General three-workflow block (2026-10-31)

Candidate `49ef4fb` used the unified full snapshots and the same production skill
for triage, document editing and an asynchronous wizard. Before execution, the
manifest fixed two paired datasets per workflow, alternating native/cu order:
12 new Codex tasks, disposable apps and state directories. Actual settings were
`gpt-6-astra / high`, no service-tier override. Each full turn includes setup.
No benchmark solver or task-specific production branch was added. Inputs stayed
frozen throughout; all timestamps confirm sequential, non-overlapping trials.

**Runtime-context limitation:** the host's create-task tool inherited the
repository working directory and automatically injected its development
`AGENTS.md`. Initial-context inspection confirmed this after the block. These
are repository-context diagnostics, **not a clean skill-only production
comparison**. The timings below describe those runs only. The actual product
integration and L2 system prompt use the skill, not development instructions.
Future comparisons follow the neutral-directory requirement above; adding a
prompt to ignore repository instructions does not remove their context cost.

Under the original block's action-isolation audit, native had 6 passed;
cu had 3 passed and 3 excluded. These counts do not waive the shared runtime
context limitation. Independent
saved-state checks passed in 11/12 attempts. The excluded document attempt saved
the right fields and order but never completed the workflow. Exclusions are
real integration failures, not evidence that cu is reliable on half the inputs.
They are retained without replacement in the [complete report](benchmarks/general-snapshot-2026-10-31.json).

| Workflow / dataset | cu full task | Native full task | Outcome |
| --- | ---: | ---: | --- |
| Triage 1 | 86.4 s | 108.0 s | both pass |
| Triage 2 | 207.9 s | 112.8 s | cu excluded: observed foreground app |
| Document 1 | 121.3 s | 142.2 s | both pass |
| Document 2 | 331.8 s | 144.5 s | cu excluded: incomplete, recovery exceeded scope; coordinator stopped it |
| Wizard 1 | 149.1 s | 173.6 s | both pass |
| Wizard 2 | 204.1 s | 148.7 s | cu excluded: observed foreground app |

There is only **one eligible pair per workflow**: cu/native full-task ratios
0.800, 0.853 and 0.859 respectively. Do not pool these into a general speed claim
or compare success-only medians across different eligible datasets. In those
pairs, recorded cu/native tool intervals were 5.3/12.7 s, 5.3/10.1 s and
9.9/21.7 s. Most full-task time remained outside tools, including model, network
and scheduling. The native document tasks each recovered from seven AX errors;
these costs remain counted. cu's menu transitions also exercised stale-ref
rejection and recovery; those checks prevented dispatch to changed targets.

Trace review identified two general problems, fixed **after** this frozen block:

- The session helper interpreted a leading output option as an unwrapped CLI
  call. `cu --compact snapshot --limit 200` therefore dropped the bound PID in
  both triage and wizard. Output options now change only presentation; the
  helper always retains its target and grouping behavior. Advanced root options
  and verified input use the explicit binary with an explicit target. Unsupported
  root options no longer silently bypass task binding.
- Standard window buttons had empty labels. After an unnecessary reopen, the
  document participant guessed that an empty button was navigation and closed
  the window. Empty strings now permit label fallback, and unlabeled buttons
  use the system role description. The skill directs unknown controls to
  existing inspection capabilities and keeps recovery inside the authorized
  task. No new retry, sleep, per-action verification, task detector or API was added.

The document recovery changed three login-session environment values. The
coordinator sent a stop instruction, removed those exact task values and stopped
the extra Broker; its 331.8 s is an interrupted unsuccessful duration, not time
to completion. All disposable fixtures and dedicated Brokers were stopped.
Full traces remain local; the report exports results, audits and evidence hashes,
not model reasoning or unrelated foreground contents.

Post-block validation: 43 Rust tests, Clippy with warnings denied, release build,
format/version checks and the affected native command suites passed (16 checks,
one TextEdit skip to preserve the existing session). Real shell tests cover
targeted reads/edits with output flags in sh, bash and zsh; the native fixture
checks button descriptions in both AX readers and closes only its second window.
The 8 offline protocol tests passed before the unchanged benchmark harness ran.
These regression checks validate the fixes; the timings above do **not** measure
the subsequent helper/label changes. This is a diagnostic sample in three modes
of one AppKit fixture, sharing the OS and caches, not proof for arbitrary apps.

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

## Scalar values and continuous triage (protocol v2)

Candidate `22f8e7a` fixes the observation gap found above. Snapshots and focused
summaries now decode text, boolean and numeric AXValue into the existing `value`
string. Checkboxes expose `0`/`1` (and mixed state `2`); integer values retain
precision rather than round-tripping through floating point. The decoder uses
the already-read batch slot or existing attribute fallback. No extra AX query,
action-effect check, retry or new CLI output field was added.

The real-control regression failed against the old binary because `value` was
absent, then passed after the fix. It toggles an owned checkbox off and on,
checks the application's independent callback label, reconstructs compact
observations, and exercises the single-attribute fallback. Rust coverage also
checks booleans, mixed state, an integer above 2^53, fractional values, Unicode
and unsupported objects. Validation passed: 44 Rust tests, Clippy with warnings
denied, release build, formatting/version checks, the native action-latency
behavior suite, and eight offline benchmark-protocol tests.

The next experiment used the same four paired triage datasets and alternating
order, but **one uninterrupted turn per task**, including initialization.
Every actual turn used `gpt-6-astra / high`, no tier override. All eight attempts
were retained; no replacement or coaching occurred, and code/inputs stayed
fixed throughout the block. The full single-turn time is the only comparable
latency metric here; do not pool it with protocol-v1 GO measurements.

**Eligible exact outcomes: cu 4/4; native 3/3, with one native attempt excluded.**
The excluded native dataset-1 task was affected by a coordinator mistake: its
app was stopped after the saved workflow said complete, while the task itself
was still active. Its task later reported DONE at 229.8 seconds. The pre-stop
saved fields already left T103/T107 Urgent incorrectly on. Both the wrong fields
and full timing remain in the artifact, along with pre-stop trace/result hashes;
it contributes neither a success nor a failure to the eligible comparison.
This is an experiment deviation, not a reason to silently replace the sample.

| Dataset | Order | cu full task | Native full task | Eligible exact outcome cu / native |
| --- | --- | ---: | ---: | --- |
| 1 | native → cu | 174.8 s | **excluded 229.8 s** | pass / excluded |
| 2 | cu → native | 177.0 s | 99.4 s | pass / pass |
| 3 | native → cu | 106.1 s | 108.8 s | pass / pass |
| 4 | cu → native | 153.7 s | 105.6 s | pass / pass |

For eligible datasets 2–4, the median paired cu/native full-task ratio is
**1.455**: cu took about **45.5% longer** in this block. This block does not demonstrate a general speed
win. The previously incorrect cu dataset-4 Urgent fields were both correct this
time, including original and correction states, but a small cross-protocol
comparison cannot establish a causal agent-accuracy gain from the value fix.

All eight tasks reported zero tool errors; none of the native tasks needed the
activation recovery seen after READY/GO in protocol v1. The task-boundary error
did not recur, but code, protocol and runtime latency also changed, so the timing
difference between blocks cannot be attributed solely to removing the boundary.
Native dataset 4 additionally listed its own trial filenames and Git status;
it did not read source/saved contents. That metadata-read cost is retained.

cu datasets 1/2/4 switched from compact output to full ordinary JSON snapshots,
returning **83–84k text characters** and generating about **4.1–4.3k output
tokens** per task. Dataset 3 retained compact observations: **38.9k characters**,
**1,961 output tokens**, and 106.1 seconds versus native's 108.8. All these counts
include initialization. Recorded cu tool intervals were only 4.9–5.3 seconds;
eligible native intervals were 11.6–13.1 seconds. Extra model interaction and
context size are useful next investigation targets, but these strategy/latency
associations are not a controlled causal comparison. In particular, requiring
full state should not force a return to verbose metadata; that is a proposed
follow-up, not a change measured in this block.

[Complete continuous-task records](benchmarks/triage-continuous-2026-10-31.json)
include all attempts, the coordinator deviation, field differences, model usage,
actual order, audit notes and evidence hashes. Traces remain local and contain
no exported reasoning. All owned apps and Brokers were stopped. Limits remain:
three eligible pairs, one custom AppKit task, shared OS/caches, uncontrolled
backend load, and differing backend instrumentation. This does not establish
browser/Electron or commercial-app performance.

## Complete concise snapshots (protocol v2 follow-up)

Candidate `ba8abaf` removes the implicit delta branch from explicit compact
snapshots. `cu snapshot` in the session helper now returns the complete concise
state within its limit, including unchanged controls and exact text. Recovering
full state no longer requires verbose ordinary JSON. Action/wait observations
still use deltas. This deletes production logic and adds no AX query, effect
check, retry, public flag or output field. Private Broker protocol 7 retires
Brokers using the old snapshot contract. The skill and command reference now
reserve ordinary JSON for geometry/paths and machine integration.

The native regression covers two grouped edits, repeated unchanged observations,
exact long Unicode values and acting on an unchanged ref from a complete compact
snapshot. It compares the complete semantic state with ordinary JSON and checks
that the compact fixture response is under 60% of its size. Checkbox action
deltas and independent on/off callbacks remain covered. Validation passed:
44 Rust tests, eight offline protocol tests, native action-latency behavior,
Clippy, formatting, version synchronization, release build and candidate CI.

The follow-up reused the same four datasets, fixture and protocol-v2 timing:
eight fresh tasks/apps, alternating order, actual `gpt-6-astra / high`, no tier
override, initialization included. Code/inputs stayed frozen, with no coaching,
replacement attempts or overlapping trials. Collection and cleanup occurred only
after every task explicitly completed. All eight saved fixture outcomes were
exact, but **eligible counts are cu 3/3 plus one exclusion, native 4/4**.

| Dataset | cu full task | Native full task | Eligible outcome cu / native |
| --- | ---: | ---: | --- |
| 1 | 99.5 s | 96.9 s | pass / pass |
| 2 | **excluded 100.3 s** | 114.7 s | excluded / pass |
| 3 | 111.9 s | 106.1 s | pass / pass |
| 4 | 154.4 s | 80.6 s | pass / pass |

The excluded cu task inserted a space inside its configuration-file path. The
subsequent untargeted click used the globally installed CLI and was rejected
with `observation_required`; the following snapshot observed a non-fixture
foreground app. Although it recovered and filled the fixture correctly, this
violated the assigned-app-only protocol. Its duration, final values and audit
remain recorded, with no replacement. The mixed shell command exited zero, so
its embedded failure is **not counted by the outer tool-error counter**. Do not
interpret that counter as proof of zero command failures. The private trace is
retained locally; unrelated app output is not exported in the report.

The three eligible paired cu/native full-task ratios have median **1.055**
(cu took 5.5% longer), ranging from 1.027 to 1.916. This cannot be advertised as
an improvement from the preceding block's 1.455: the eligible datasets differ.
On the **common datasets 3 and 4**, the median ratio was **1.215 before and
1.486 now**, which does not demonstrate a consistent speedup.

| Eligible cu dataset | Returned characters | Model calls | Output tokens | Recorded tool time |
| --- | ---: | ---: | ---: | ---: |
| 1 | 40,229 | 24 | 3,646 | 5.1 s |
| 3 | 39,102 | 23 | 1,978 | 5.0 s |
| 4 | 40,209 | 25 | 3,688 | 4.7 s |

All eligible cu tasks retained compact output, removing the ordinary-JSON
fallback seen in the previous block. Datasets 1 and 4 now returned about half
as much text (previously 83,173 and 84,211 characters). Dataset 3 had already
stayed compact, so its output size barely changed. The excluded task's fixture
observations were also compact; its accidental unrelated read is not evidence
of an intended fallback strategy.

The slow fourth cu task still spent 149.6 seconds outside recorded tool
intervals. Native dataset 4 was fastest despite reading the general skill and
Git status metadata, then requesting full AX states with `disableDiffing`.
Those reads were not fixture/source/result reads and their costs remain counted.
Smaller output is established; stable wall-time improvement is not. Model
strategy, generation and backend latency still vary, and non-tool time cannot
be attributed to inference alone. No additional runtime validation or retry was
added to chase this variation.

[Complete follow-up records](benchmarks/triage-complete-snapshot-2026-10-31.json)
retain all eight attempts, settings, hashes, audits and common-dataset ratios.
All fixture apps and dedicated Brokers were stopped. Scope remains one custom
AppKit task, three eligible pairs and shared OS/caches; these measurements do
not establish general browser, Electron or commercial-app performance.
