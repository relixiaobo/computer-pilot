# L2 task discovery repair — 2026-11-03

The release runner now discovers only `tests/agent/tasks/e2e/*.json`. The three
existing release tasks were moved unchanged; benchmark recipes retain their
existing paths and harnesses. All selected tasks are loaded and checked for
nonempty `id`, `name`, `goal`, and `verify` before fixtures or model calls begin.
Invalid explicit inputs fail with their filename. No task is silently skipped.

The reproduced default `--dry-run` crash (`KeyError: 'id'`) is fixed. The repair
is based on `2cf0561`; no CLI source, release version, tag, or stable branch changed.

## Verification

- Default dry-run lists exactly the three release tasks.
- Python benchmark/runner tests: **39 passed**, including five new discovery
  regressions (default discovery, invalid later input before any execution,
  explicit benchmark rejection, explicit release task, empty directory).
- `scripts/verify.sh --skip-desktop`: version/metadata, formatting, Clippy,
  **46 Rust tests**, and release build passed.
- Affected live suite `run_all.sh agent_cleanup`: **6 passed, 0 failed, 0 skipped**.
  It constructed duplicate app fixtures and verified their removal using the
  moved task files. The full L1 suite was not rerun; prior full-suite evidence
  remains in the final candidate acceptance report.
- `git diff --check`: passed.

## Formal L2 result: blocked by provider quota

`python3 tests/agent/run.py` loaded all three tasks and passed credential
preflight. Each first model request failed with HTTP **429**,
`USAGE_LIMIT_EXCEEDED` / `WEEKLY_LIMIT_EXCEEDED`. The runner completed with exit
code **1** and **0/3 verified**. No model reply or agent action was produced;
fixture preparation, verification and cleanup did run. No cleanup error was
reported. This is neither a behavioral L2 pass nor evidence of an agent regression.

Local results: `test-results/agent-1793697699.json`. Raw logs remain local because
verification can include user application data. Their hashes are in the JSON
companion. Publication remains pending a successful formal L2 run with an
available provider quota; rerun the default command after that is restored.
