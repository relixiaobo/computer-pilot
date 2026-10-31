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
