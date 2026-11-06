# Action limit audit and extended acceptance — 2026-11-06

Local candidate on `codex/action-limit-audit`, based on `b922ad9` (v0.9.5).
Dates use Asia/Shanghai. This is maintainer-driven CLI/desktop verification,
not a fresh-model L2 cohort, an API-runner pass, or release acceptance.
The later [formal API L2 run](2026-11-06-action-limit-l2.md) is recorded
separately: initially 2/3 verified with Mail blocked before agent execution;
the unchanged mail-only rerun subsequently passed 1/1 after scripting recovered.

## Reproduced defect and repair

A `set-value` target under AXWebArea at ref 132 was observed with limit 200.
Writing through that ref with `--limit 1` succeeded, but silently omitted
`controlled_editor_risk` and `effect_advice`, and reported medium rather than
low confidence. The same control at limit 200 carried the expected advisory.
The new regression failed against the pre-fix binary before the repair.

`snapshot_for_ref_action` correctly retained the AX object during the larger
freshness scan, then restricted its pre-action snapshot to the action limit.
`cmd_set_value` looked for the target's web ancestry in that restricted prefix.
The repair requests enough pre-action metadata to include the selected ref.
The existing Observation already covers it, so this adds neither a tree read
nor a larger public freshness scan. Post-action output still uses the original
requested limit. Broker transport and target freshness rules are unchanged.

The owned AppKit fixture models AXWebArea ancestry and an editable AX value.
Its tests verify actual value changes, preserved risk advice at limits 200/1,
and compact output with `--no-snapshot`. It does not model browser JavaScript
handlers or establish that an AXValue write commits a controlled web editor.

## Verification runs

| Run | Result | Interpretation |
| --- | --- | --- |
| Initial `bash scripts/verify.sh` | Rust/metadata/build passed; L1 **785 passed, 24 failed, 29 skipped** across 63 suites | Not a passing full gate; executed before the runtime repair |
| Python benchmark checks | **47 passed** | Existing reporting checks |
| New advisory regression before repair | Failed at limit 1 after passing limit 200 | Retained reproduction, not erased by later success |
| `bash scripts/verify.sh --skip-desktop` after repair | Passed, including **46 Rust tests**, formatting, Clippy and release build | Final runtime source validated |
| Complete `action_latency` suite after repair | Passed, no skips | Includes new limit/advisory cases and existing input, identity, recovery and snapshot checks; the shell reports one aggregate assertion |
| Nine initially failing suites rerun after repair | **132 passed, 0 failed, 3 skipped** | Targeted recovery, not another full L1 run |

The nine rerun suites were `agent_cleanup annotated click key ocr
ocr_confidence perform ref_projection tell`. Initial failures included absent
Finder window/Observation state, TextEdit fixture initialization and
Calendar/Reminders AppleScript timeouts. Later script reads succeeded; the
cleanup suite's real fixtures and postconditions passed on rerun. These runs
do not establish one root cause for every initial failure.

Two rerun skips preserved an already-running TextEdit session (`key` and the
TextEdit portion of `ref_projection`). The third was a missing action-less
Finder target in refs 1–8. Foreground/global-input cases remained disabled.
Unaffected passing full-run checks were reused. The initial full run remains
failed; the scoped rerun does not retroactively turn it green.

## Actual application use

All UI actions used explicit PID selectors. The agent used `cu tell` for
document saving, application-level checks and owned-window cleanup. No
foreground activation or global input was used for these scenarios.

| Scenario | Observed result |
| --- | --- |
| TextEdit ref write with limit 1 | Chinese, emoji and final newline saved exactly; independently checked UTF-8 file bytes |
| TextEdit stale Observation write | Rejected before dispatch; recovery state supplied |
| TextEdit path write with limit 1 and no snapshot | Exact final file content saved, 65 bytes |
| TextEdit click limit 1 → Cmd+A → Unicode replacement | **Exact-result check failed**: Cmd+A did not select all in the background; input was inserted before the original text. Saved failure artifact retained |
| TextEdit click limit 1 → Unicode insertion into an empty document | Focus confirmed; `unicode-pid`, `value_changed:true`; exact saved content including one final LF, 63 bytes |
| Finder high-ref filename click, limit 1 vs 200 | AX accepted both clicks but scripting returned an empty selection in both. Both receipts correctly reported `verified:null` and advised inspecting the unchanged state |
| Finder script-first selection of the owned file | `cu tell` selected the exact file; independent scripting read returned `{"limit-item-07.txt"}` |

The first Finder oracle used an invalid list-property expression and failed.
It was corrected to iterate the returned selection. Subsequent attempts and
the 1-versus-200 comparison still found no selected file. This is an observed
background interaction limitation, not evidence of a limit-specific regression.
Similarly, generic key dispatch and `value_changed:true` never established that
Cmd+A selected the intended text or that the final document was correct.
Whole-field replacement through `set-value` and script-first Finder operations
passed; global input/focus were not used to force the failing paths through.

Every scenario closed its own document/window and verified absence. User
documents were preserved. The rerun cleanup suite verified removal of its
Calendar, Reminders, Notes and desktop fixtures. Saved artifacts and traces
remain under the ignored `test-results/action-limit-2026-11-06/` directory.

## Evidence and reproduction

Local raw evidence:

- `verify.log`, `verify-after-fix.log`, `python.log`
- `advisory-regression-before.log`, `advisory-regression-after.log`
- `failed-suites-retest.log`
- `live_usage.py`, `live-trace.jsonl`, `live-results.json`, `finder-retest.json`
- `finder-comparison.json`, `finder-script-results.json`
- `keyboard-live-results.json`, `unicode-live-results.json`, `source-hashes.json`

Final tested binary SHA-256:
`b501b33d07352c0b0a7d5332939ae69f83eb2ddccf9f193e994eb1f5d2835329`.

Saved TextEdit artifacts:

| Artifact | SHA-256 |
| --- | --- |
| `cu-limit-real-20261106.txt` (pass) | `3fbfcb232b6465c37103e5f4e45f34203e5ad65bc592842a8c63b5868bc0d28b` |
| `cu-limit-keyboard-20261106.txt` (failed replacement) | `aaa08b467f4fb1962c55dcfbc503e4fcf5be8e4c05d768c812741fb3ad104c32` |
| `cu-unicode-insertion-20261106.txt` (pass) | `82238152172b6c34f5aa441fc08d0aaa229234389dcc87edbffbfe26c112f75c` |

Reproduce the maintained regression with:

```bash
cargo build --release
bash tests/commands/run_all.sh action_latency
```

The targeted defect is fixed and covered. The evidence does not establish
general background interaction reliability, browser editor commit behavior,
full interactive coverage or a new L2 release pass.
