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
