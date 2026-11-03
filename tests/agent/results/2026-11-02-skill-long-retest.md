# Long-task skill retest — 2026-11-02

12 planned Computer Pilot attempts across six cases, two repetitions each. Exact saved artifacts: **3/12**. Completed within the deadline with exact artifacts: **2/12**. Timeouts: **7**. Model-service failures: **1** (reported separately from product accuracy). One pre-model setup failure is recorded separately; no model failure was replaced.

Completion claims: 2; false claims: 0. An interrupted turn without a completion claim is not evidence of successful self-verification.

These are **shared-session results**, not a clean-desktop causal speed comparison. Product and skill were frozen; the only mid-cohort change repaired coordinator clipboard compatibility. Native results are historical and were not rerun.

| Attempt | End state | Exact files | Turn s | Tool calls | Errors | Tool s | First exact s |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| finder-organize-r2-cu | timeout | no | 599.7 | 31 | 3 | 16.1 | — |
| finder-rename-r2-cu | failed | no | 107.4 | 14 | 0 | 5.8 | — |
| finder-rename-r1-cu | failed | no | 142.2 | 13 | 1 | 8.8 | — |
| finder-organize-r1-cu | timeout | no | 599.8 | 28 | 1 | 12.0 | — |
| textedit-2-cu | timeout | no | 599.8 | 24 | 4 | 8.4 | — |
| textedit-1-cu | timeout | no | 599.8 | 17 | 0 | 6.7 | — |
| vscode-markdown-r1-cu | timeout | yes | 598.7 | 21 | 3 | 7.2 | 539.6 |
| vscode-multi-r2-cu | timeout | no | 598.1 | 25 | 6 | 6.6 | — |
| vscode-multi-r1-cu | timeout | no | 597.8 | 14 | 1 | 5.3 | — |
| vscode-json-r2-cu | passed | yes | 193.7 | 7 | 0 | 5.5 | 171.1 |
| vscode-json-r1-cu | infrastructure_failed | no | 525.6 | 4 | 0 | 0.7 | — |
| vscode-markdown-r2-cu | passed | yes | 448.1 | 10 | 0 | 9.2 | 295.1 |

Tool calls count recorded outer command/tool executions; CLI calls inside a shell group are separate backend-call estimates in the JSON. Timeout rows use interruption duration, not the raw collector’s zero.

Historical controls (seconds / outer calls; all retained):

| Cohort / attempt | Result | Seconds | Calls |
| --- | --- | ---: | ---: |
| codex-matrix-fix-20261102-finder / finder-rename-r1-cu | failed | 71.9 | 13 |
| codex-matrix-fix-20261102-finder / finder-rename-r1-native | passed | 264.1 | 36 |
| codex-matrix-fix-20261102-finder / finder-organize-r1-cu | passed | 576.3 | 68 |
| codex-matrix-fix-20261102-finder / finder-organize-r1-native | passed | 210.3 | 40 |
| codex-matrix-fix-20261102-textedit2 / textedit-1-cu | passed | 139.7 | 30 |
| codex-matrix-fix-20261102-textedit2 / textedit-1-native | failed | 123.6 | 15 |
| codex-matrix-fix-20261102-vscode2 / vscode-json-r1-cu | passed | 157.6 | 31 |
| codex-matrix-fix-20261102-vscode2 / vscode-markdown-r1-cu | passed | 130.5 | 23 |
| codex-matrix-fix-20261102-vscode2 / vscode-markdown-r1-native | passed | 38.8 | 3 |
| codex-matrix-fix-20261102-vscode2 / vscode-multi-r1-native | passed | 88.8 | 10 |
| codex-matrix-fix-20261102-vscode2 / vscode-json-r1-native | passed | 22.5 | 2 |
| codex-matrix-fix-20261102-vscode2 / vscode-multi-r1-cu | passed | 209.5 | 31 |
| codex-matrix-fix-20261102-vscode-markdown3 / vscode-markdown-r1-cu | passed | 122.9 | 19 |
| codex-matrix-fix-20261102-vscode-markdown3 / vscode-markdown-r1-native | passed | 128.0 | 15 |

Observed execution findings:

- Routing improvement: completed traces use the assigned CLI/skill UI route; VS Code attempts avoid the earlier unsupported scripting probes. No alternate backend or direct fixture edits were accepted.
- Finder: both organize attempts timed out; both rename attempts reported blocked. PID double-clicks and menu actions could be accepted without the expected navigation or folder creation. One organize attempt navigated with keyboard input. One rename attempt ended with explicit focus_failed. Stale guards prevented use of refs whose targets had changed.
- TextEdit: both attempts timed out. One final file missed only one checklist toggle; the other lost contact text and an archived owner field, duplicated Contacts and missed both toggles. A new Find/Replace field value followed by All produced a document change inconsistent with the new replacement text; field state, application handlers and commit timing need a targeted reproduction before assigning root cause.
- VS Code: repeated focus loss, stale targets, redundant observations and controlled-editor set-value fallbacks consumed recovery calls. At least one participant continued keyboard/input after focus_failed despite the skill instruction. These are skill-adherence failures even when backend restrictions were respected.
- Accuracy regression: multi-r1 service.toml was byte-exact after Save, then an unnecessary rewrite removed its final newline, then Enter/Save repaired it. AX boundary text was treated as saved-byte evidence despite explicit skill guidance. Do not optimize this by removing final artifact checks.
- Markdown-r1 saved an exact file roughly nine minutes into the turn, but never completed before timeout. No tool calls completed after the first exact save; the remaining interval must not be described as extra verification tool calls.
- Markdown-r2 used grouped local edits, preserved exact bytes and completed; its account-concurrency reconnect after Save inflated total turn time without adding document edits.
- JSON-r2 completed with exact bytes in 193.708 seconds, 7 outer tool calls and zero tool errors, versus historical CU 157.594 seconds/31 calls and native 22.467 seconds/2 calls. Fewer calls alone did not imply lower total latency.

Recommended next work, in order:

1. Next comparison: use a dedicated macOS login session/desktop and defined initial foreground/Find state, while retaining fresh model contexts and fixtures. Keep these shared-session attempts in the record; do not relabel them as isolated-desktop runs.
2. Reproduce the general input lifecycle across apps: observed target, focus, shortcut/selection, text delivery, committed application value. Diagnose background delivery separately from native AX activation and genuine foreground requirements.
3. Reproduce native Find/Replace field commit behavior and document selection before changing batching rules. Batch only known operations whose target and application state remain established; do not add a sleep or app-specific solver.
4. Treat AX text presentation and saved-file bytes as different evidence. Investigate why agents still rewrite correct boundary newlines; prefer a clearer tool/result contract and fewer conflicting rules over more repeated prose.
5. Keep identity, focus, clipboard and outcome guarantees. Eliminate redundant help, snapshots and unsupported route switches in recovery, and distinguish dispatch failures from unknown outcomes before retrying.
6. Standardize the collector to preserve interrupted timing and per-file exact-to-inexact transitions in its normal report; this report already accounts for interrupted time and explicitly documents the observed regression.

Protocol and limits:

- Fresh model profiles, threads, neutral working directories, fixtures, CU runtimes and clipboard isolation; project documents and memories disabled. Runtime audits confirmed no AGENTS.md injection. Finder/TextEdit/VS Code processes and the macOS desktop were shared, not isolated OS sessions.
- Foreground changes were observed, including focus_failed with another application frontmost. TextEdit Find state persisted across shared-app documents. These are shared-session execution results, not a controlled estimate of a skill-only or backend-only speed change.
- One JSON attempt encountered model-server overload and exhausted retries with HTTP 429 before editing. It remains in the planned-attempt record but is an infrastructure failure, not evidence of a product accuracy defect. Finder organize-r1 and Markdown-r1 also logged stream reconnection errors; multi-r1 logged a reconnection and overload. Markdown-r2 hit an account concurrency limit after its exact save, then recovered and completed. Rows preserve all service error messages. These timing confounds prevent a pure product-speed conclusion.
- Same gpt-6-astra/high/null service-tier settings as historical controls. Native Computer Use was not rerun. All historical rows, including both Markdown rounds, are retained; they are non-contemporaneous references, not new paired measurements.
- Each model attempt had a 600-second CLI wall timeout. Reported time is the completed or interrupted model task turn, including skill discovery, but not CLI startup before task_started. Timeouts remain unsuccessful deadline completions even if saved artifacts are exact; artifact accuracy is also reported independently.
- Interrupted turns have combined_turn_ms=0 in the existing raw collector. This report reconstructs metrics from the original rollout using interrupted_elapsed_ms, never treats those zeros as measured duration, and retains completed tool/usage records. In-flight work may not be represented in token/tool counters.
- Tool intervals are the union of recorded tool executions. Residual time includes model, network and orchestration, not pure inference. Backend invocation estimates may count unexecuted lines in failed shell batches.
- The first Finder organize timeout was retained. A later Finder rename setup failed before model launch because a legacy clipboard type was unreadable through NSPasteboardItem; it did not clear the clipboard. Only coordinator clipboard IO changed. Successor manifests preserved the original remaining order, product/skill fingerprints and failed evidence; no failed model attempt was replaced.
- No coaching or product/skill changes during the cohort. Raw rollouts, UI output, clipboard backups and credentials remain private. Cleanup closed only dedicated test windows. Clipboard restore skipped when changeCount changed and current contents were not recognized as task-owned files; that guard cannot distinguish user writes from tool clipboard activity. Original clipboard backups remain private; no later clipboard state was overwritten to force restoration.

The clipboard coordinator regression test uses a temporary named NSPasteboard, covers legacy aliases/binary data/multiple items/user changes, and never replaces the general clipboard. All 18 real-app harness tests passed. Product unit/command gates from the frozen commit remain applicable; no product code changed in this retest.

[Machine-readable results](2026-11-02-skill-long-retest.json) include model settings, fingerprints, timing, counters, oracle checks, audits and evidence hashes; no raw clipboard or UI contents are published.
