# Resource-based long-task retest — 2026-11-02

Six cases, two fresh agent attempts each. One pre-model setup failure is recorded separately. **7/12 passed within budget**, **7/12 exact saved artifacts**. Task failures: 3; budget stops: 1; infrastructure failures: 0; protocol violations: 2 (these dispositions can overlap). **False completion claims: 2 of 9 claims.**

Primary measures are correctness, model rounds and token consumption. Each attempt receives 100 model responses and 8 million cumulative tokens. The 600-second task cutoff is removed. Elapsed time is diagnostic only.

| Attempt | Result | Exact | Rounds | Total tokens | Uncached input | Cached input | Output | Reasoning | Tool calls |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| finder-organize-r2-cu | failed | no | 94 | 7284117 | 608468 | 6663168 | 12481 | 6724 | 88 |
| finder-rename-r2-cu | passed | yes | 30 | 1253985 | 64938 | 1186688 | 2359 | 821 | 28 |
| finder-rename-r1-cu | failed | no | 33 | 1592491 | 243031 | 1343872 | 5588 | 2423 | 29 |
| finder-organize-r1-cu | passed | yes | 71 | 4805720 | 287971 | 4509696 | 8053 | 4042 | 75 |
| textedit-2-cu | passed | yes | 16 | 529996 | 37386 | 490752 | 1858 | 724 | 15 |
| textedit-1-cu | passed | yes | 55 | 2356694 | 254476 | 2094848 | 7370 | 3968 | 51 |
| vscode-markdown-r1-cu | passed | yes | 37 | 2064772 | 284113 | 1770880 | 9779 | 6668 | 33 |
| vscode-multi-r2-cu | excluded (budget_exhausted) | no | 72 | 8150092 | 1274660 | 6860800 | 14632 | 8654 | 71 |
| vscode-multi-r1-cu | failed | no | 30 | 1746550 | 289215 | 1451136 | 6199 | 3543 | 26 |
| vscode-json-r2-cu | passed | yes | 60 | 6560364 | 679886 | 5867904 | 12574 | 7094 | 58 |
| vscode-json-r1-cu | excluded (scope_violation) | no | 51 | 4480279 | 885142 | 3580416 | 14721 | 8471 | 49 |
| vscode-markdown-r2-cu | passed | yes | 10 | 329123 | 39599 | 285824 | 3700 | 1975 | 9 |

Input = uncached input + cached input. Output already includes reasoning; do not add that column again. A missing reasoning counter is unknown, not zero. Tool calls may exceed model rounds because a response can issue multiple tools.

Historical paired-pilot controls (not rerun; all rows retained):

| Attempt | Result | Exact | Rounds | Total tokens | Uncached input | Cached input | Output | Reasoning | Tool calls |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| codex-matrix-fix-20261102-finder / finder-rename-r1-cu | failed | no | 14 | 389746 | 32937 | 355968 | 841 | 117 | 13 |
| codex-matrix-fix-20261102-finder / finder-rename-r1-native | passed | yes | 37 | 1997666 | 86420 | 1904640 | 6606 | 4487 | 36 |
| codex-matrix-fix-20261102-finder / finder-organize-r1-cu | passed | yes | 76 | 5414300 | 136295 | 5260800 | 17205 | 9356 | 68 |
| codex-matrix-fix-20261102-finder / finder-organize-r1-native | passed | yes | 41 | 1668498 | 51126 | 1612544 | 4828 | 2823 | 40 |
| codex-matrix-fix-20261102-textedit2 / textedit-1-cu | passed | yes | 31 | 1082679 | 45368 | 1035264 | 2047 | 506 | 30 |
| codex-matrix-fix-20261102-textedit2 / textedit-1-native | failed | no | 16 | 467554 | 22859 | 440320 | 4375 | 2780 | 15 |
| codex-matrix-fix-20261102-vscode2 / vscode-json-r1-cu | passed | yes | 32 | 2023771 | 110251 | 1909248 | 4272 | 2386 | 31 |
| codex-matrix-fix-20261102-vscode2 / vscode-markdown-r1-cu | passed | yes | 24 | 1296453 | 83800 | 1208448 | 4205 | 2539 | 23 |
| codex-matrix-fix-20261102-vscode2 / vscode-markdown-r1-native | passed | yes | 4 | 106357 | 15500 | 89984 | 873 | 451 | 3 |
| codex-matrix-fix-20261102-vscode2 / vscode-multi-r1-native | passed | yes | 11 | 350461 | 30244 | 318080 | 2137 | 1302 | 10 |
| codex-matrix-fix-20261102-vscode2 / vscode-json-r1-native | passed | yes | 3 | 75016 | 24636 | 50048 | 332 | — | 2 |
| codex-matrix-fix-20261102-vscode2 / vscode-multi-r1-cu | passed | yes | 32 | 2143622 | 111480 | 2027776 | 4366 | 2227 | 31 |
| codex-matrix-fix-20261102-vscode-markdown3 / vscode-markdown-r1-cu | passed | yes | 20 | 649419 | 27878 | 619136 | 2405 | 284 | 19 |
| codex-matrix-fix-20261102-vscode-markdown3 / vscode-markdown-r1-native | passed | yes | 16 | 777805 | 59730 | 715136 | 2939 | 1403 | 15 |

Execution findings:

- Finder organize-r2 reached 94 rounds and 7,284,117 tokens. Both files moved, but archive.txt remained unrenamed; the agent claimed that rename was verified. The large response/token allowance did not ensure honest completion.
- Finder rename-r2 passed using focused keyboard editing in 30 rounds / 1,253,985 tokens. Rename-r1 used AX set-value and accepted AXConfirm; both AX text and a retained screenshot showed changed labels, yet disk paths remained original. It falsely claimed completion at 33 rounds / 1,592,491 tokens. Re-observing mutable UI state is not always evidence of a committed operation.
- Finder organize-r1 passed in 71 rounds / 4,805,720 tokens after 13 tool errors. The historical native success used 41 rounds / 1,668,498 tokens. Compare successful completion, not cheap incomplete attempts.
- TextEdit-2 passed with exact bytes, no tool errors and a supported completion claim in 16 rounds / 529,996 tokens. Find plus keyboard selection/editing and cut/paste preserved the document. The historical CU success used 31 rounds / 1,082,679 tokens; historical native used 16 rounds / 467,554 tokens but failed exact newline preservation.
- TextEdit-1 also passed, but required 55 rounds / 2,356,694 tokens. It first changed repeated text in Archived window, then repaired it and finished after grouping explicit target activation with Find/edit. The attempt contains focus/no-effect recovery, two unviewed full-screen captures, ungrounded coordinate attempts and three service reconnect/overload events. Both TextEdit samples are correct, but their token consumption differs by more than fourfold; a single fast run is not a reliability estimate.
- Markdown-r1 passed in 37 rounds / 2,064,772 tokens after focus, stale-ref and controlled-editor recovery. It used whole-document reconstruction despite the skill guidance. Saved bytes were already exact when it announced an extra trailing newline; it only observed/clicked/captured afterward, so the suspected regression did not occur. Historical native successes used 4 rounds / 106,357 tokens and 16 rounds / 777,805 tokens. Five service errors also affected this attempt.
- A concrete observation-contract gap appeared in Markdown-r1: state --no-screenshot returned 50 elements and tree_truncated:true without an actionable string advisory, replacing the client observation. The next set-value reused prior ref 73 and was rejected as absent. The identity guard worked; clearer truncation output and avoiding unnecessary state replacement are general follow-up targets.
- Multi-r2 reproduced contradictory focus receipts within one shell group: click reported focus_verified:true, then type refused with dispatched:false and focused:null. Static inspection found different evidence sources: attach_text_input_focus reads the retained target AXFocused flag, while focused_input_before_type reads the application AXFocusedUIElement from a snapshot. Unify the meaning of confirmed input focus using existing observations before considering removal of the pre-dispatch guard; this mismatch is a concrete general follow-up, although temporal focus changes still need a controlled reproduction.
- Multi-r2 exhausted its token budget at 72 rounds / 8,150,092 tokens with 21 failed outer tool calls and both saved files unchanged. It then also failed the protocol audit: after closing the assigned window, its last snapshot read an older benchmark window. A stale-ref rejection prevented the attempted write there. The row is excluded for scope violation, with the budget stop and full consumption retained independently; it is not replaced. This exposes the limits of a PID-only task binding when multiple windows share an app.
- Multi-r1 stopped honestly as blocked after 30 rounds / 1,746,550 tokens, also leaving both saved files unchanged. It repeated controlled-editor AX writes and focus/typing recovery, used unviewed screenshots and a generic get-version scripting probe. Both multi-file repetitions failed; the historical native success used 11 rounds / 350,461 tokens. No successful CU/native efficiency ratio can be claimed for these current attempts.
- JSON-r2 passed in 60 rounds / 6,560,364 tokens with 15 failed outer tool calls, compared with the historical native success at 3 rounds / 75,016 tokens. Failed focus recovery, AX-value writes, unsupported scripting probes and transport switches dominated the trace. Grouping explicit activation, selection and clipboard input eventually succeeded. Six service errors affected timing, but do not explain away the recorded 60 decision rounds.
- JSON-r1 used 51 rounds / 4,480,279 tokens without changing the saved file. It closed the task window during recovery and later observed an older benchmark window; the added coordinator scope observer stopped it immediately after that observation. This is a second scope violation, retained as excluded with consumed resources rather than an efficiency result. Neither PID delivery nor ref freshness alone binds a long task to one of several application windows.
- Markdown-r2 passed in 10 rounds / 329,123 tokens with no tool or service errors. It used grouped local Find/keyboard edits and preserved exact bytes without a whole-document rewrite. The other current Markdown success used 37 rounds / 2,064,772 tokens with the same frozen product/skill. This shows a workable low-round path and substantial run-to-run variability, not a causal speed improvement.
- Finder organize-r1 grew from 17,747 input tokens on its first response to 118,407 on its final response. Its 4,805,720 total tokens include 4,509,696 cached input tokens and 8,053 output tokens. Repeated recovery increases both decision count and accumulated context; total tokens are not newly generated reasoning or a uniform cost measure.
- A shorter elapsed duration can hide expensive recovery, while a slow model response can make a low-round task look inefficient. The new tables expose these as separate quantities without calling all residual wall time model reasoning.

Protocol and interpretation:

- Budget stops, scope violations and saved-artifact outcomes are independent dimensions; a scope-excluded attempt can also exhaust its budget. Accuracy precedes efficiency. Every attempt is retained; failed or incomplete low-consumption runs are not successful efficiency wins. No overall ranking is inferred from heterogeneous tasks or two repetitions.
- Metrics cover the participant task; coordinator fixture setup, passive auditing and report preparation are outside both arms. One model round is one recorded response ID, including discovery, recovery and final responses. Tool calls and CLI actions inside shell batches are separate counters. Tokens count cumulative recorded input plus output. Cached input is a subset of input; reasoning is a subset of output. Uncached input is reported separately; no artificial cost weighting or double counting.
- Each task has a uniform 100-response / 8,000,000-total-token budget, disclosed in the brief. The runner polls completed usage once per second; a response may overshoot. Actual consumption is retained. There is no task wall-time deadline. The 1,200-second no-activity watchdog only classifies an infrastructure interruption.
- Fresh profiles, threads, neutral cwd, fixtures, CU runtimes and clipboard isolation; project docs/memories disabled and no AGENTS.md injection verified. The same gpt-6-astra/high/null-tier settings were used. Product and skill remained frozen throughout. Finder/TextEdit/VS Code and the macOS desktop remained shared, including foreground and Find state; these were not isolated OS sessions.
- Native Computer Use was not rerun. All earlier paired-pilot reference rows are retained, including both Markdown pairs. Historical completed counts use the same recorded-response/token definition, but historical tasks did not receive the new resource-budget brief. Comparisons are observational, not a causal claim that unchanged product code became faster.
- The previous 600-second cohort is retained separately. Its interrupted records are lower-bound consumption before the time cutoff and do not establish failure under the new resource budget. A new attempt never replaces a historical failure.
- Service errors remain separate from product outcomes. Counters cover recorded completed responses; any unreported in-flight usage is unknown, not zero. Wall time is diagnostic and is not used to rank these trials.
- Legacy post_save_tool_calls counts calls ending after the first exact sample, so it can include the Save call already in flight. It is diagnostic, not a count of unnecessary later decisions. Saved-state sequences separately record observed exact-to-inexact regressions.
- After Multi-r2 left its assigned window, a coordinator-only observer was added during Multi-r1 initialization and remained active for the final four attempts. It reads completed tool output once per second and interrupts on an explicit non-modal snapshot window title outside the assigned workspace. It gives no model hints and changes no product/skill code. Any such stop remains a protocol failure with consumption at interruption, not a replacement attempt; it cannot prevent the first out-of-scope read or prove unreported window ownership.
- One pre-model setup failed because AppKit could not read invalid-UTI legacy rich-text clipboard flavors. The clipboard was not cleared and the opened test window was closed. A read-only Carbon probe could read those flavors; the user independently changed the clipboard before the fresh setup succeeded. No implementation changed mid-cohort. The setup evidence is archived separately and does not replace a failed model attempt.
- Saved-artifact sampling is coordinator-only and never coaches the agent or stops a run early. Cleanup closed only owned windows. Clipboard restoration conservatively skips unrecognized intervening writes; it cannot attribute every change to the user versus tools. Raw UI logs, clipboard backups, profiles and credentials remain private.

Recommended follow-up: diagnose committed application effects separately from AX/display writes; preserve target/focus guarantees; reduce repeated no-effect recovery and unsupported route switching. Validate general behavior across apps instead of adding task-specific solvers.

Verification: 36 benchmark/protocol tests passed, covering partial/duplicate usage records, both budgets, final responses at the cap, no 600-second cutoff, inactivity classification, interrupted usage and report status. Live runs retain model/context audits, exact file checks and cleanup evidence.

[Machine-readable results](2026-11-02-resource-retest.json) include all resource counters, limits, dispositions and evidence hashes. [The earlier wall-limited cohort](2026-11-02-skill-long-retest.md) remains unchanged.
