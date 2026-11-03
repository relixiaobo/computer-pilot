# Focus, recovery and benchmark fixes — 2026-11-03

The unreleased candidate on `fix/retest-focus-and-metrics`, based on `85c29ab`,
addresses three findings from the
[preceding TextEdit retest](2026-11-03-textedit-boundary-retest.md).
Production code and benchmark inputs were frozen before the new cohort.

**All three fresh candidate attempts passed exactly, with three supported
completion claims and zero tool errors. The broad release gate is still not
green; no release was prepared.**

## Repairs and reproductions

1. **Refresh foreground state while waiting for activation.** The old polling
   loop slept without advancing the main run loop. A bundled native fixture
   that accepts AX focus requests without activating itself reproduced a
   `focus_failed` receipt even though a separate fresh observer confirmed the
   requested PID was actually frontmost. The worker now services the default
   run loop inside the existing 1.5-second deadline. This follows AppKit's
   documented refresh behavior for time-varying application properties
   ([Apple documentation](https://developer.apple.com/documentation/appkit/nsrunningapplication)).
   It retains exact-PID verification and makes no additional activation or
   global-input attempt. The behavior test fails against the official CLI and
   passes against the candidate: six alternating activations of two same-name,
   same-bundle processes are verified independently, with pointer preservation
   and an invalid-window rejection. This reproduces a real false-negative
   class; it does not prove the cause of the original UserNotificationCenter
   outcome.
2. **Register input-error recovery snapshots.** `unknown_outcome` and pre-input
   `verification_failed` exposed diagnostic refs without registering that tree
   as the client's latest observation. A fixture changes DFS ordering between
   the previous observation and the error's snapshot. Before the fix, using
   the new ref immediately is rejected as stale. After the fix, the returned
   state repairs the intended field directly and preserves the other field.
   A separate assertion uses the pre-input error's observation to restore
   actual input focus. Failed snapshots are not published. The error hint
   now directs callers to existing diagnostic state before obtaining another
   snapshot.
3. **Count image-view operations.** The collector now includes completed
   `ImageView` records, retaining path/timestamps without exporting image
   payloads. It does not double-count the lower-level raw function-call record.
   Offline tests cover overlap, interrupted turns, post-save calls and error
   counts. Replaying the retained preceding cohort yields **5, 11 and 21**
   tool calls; the third previously reported only 15 shell calls. Its six
   image views are now included, while response and token totals are unchanged.
   Original cohort records and historical failures were not rewritten.

Independent code review found no blocking issue. No Broker protocol bump is
required: request/response formats and side-channel interpretation are unchanged;
the worker publishes error observations through an already-supported path.

## Fresh agent evidence

| Attempt | Exact saved result | Completion claim | Responses | Total tokens | Shell / image calls | Errors |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| textedit-1-cu | passed | supported | 12 | 438,437 | 10 / 1 | 0 |
| textedit-2-cu | passed | supported | 10 | 295,619 | 9 / 0 | 0 |
| textedit-3-cu | passed | supported | 10 | 258,272 | 9 / 0 | 0 |

Totals: **32 model responses, 29 tool calls, 992,328 tokens**. Input was
988,361 tokens (900,224 cached, 88,137 uncached); output was 3,967 (including
1,053 reasoning tokens). Subsets are not added again to totals.

Independent full-trace audits found all three attempts compliant with the
UI-only protocol. Every final Save-returned AX value and independently read
saved file matches the frozen oracle: **466 UTF-8 bytes and exactly one final
LF**, SHA-256 `b334b3427d16d7a56a3166403b1831b284b6b8bddecccad1ed73d83cdaa3a9aa`.
Contacts moves included their preceding separator. Scoped edits, Archived
content, tickets, Unicode and checkbox states were preserved as requested.
Passive sampling found exact saved state before each final completion response
and retained exact final samples.

Each attempt used a fresh profile, thread, neutral cwd, fixture, client key and
Broker runtime. Actual contexts match `gpt-6-astra / high / null tier`, with no
detected AGENTS injection. Budgets remained 100 responses and 8,000,000 tokens
per attempt, without a task wall-time deadline. All attempts were retained;
there were no replacements or mid-cohort fixes. Owned windows were closed and
private Brokers stopped after collection.

This is a local Rust release build of an **unreleased working tree**, still
reporting version 0.9.3, not the official release binary. Candidate source and
binary hashes are retained in the JSON report and local run archive. The
official installed binary was preserved. Environment: macOS 26.5.1 (25F80),
TextEdit 1.20 with Chinese UI, Codex CLI 0.166.0-alpha.5.1, Python 3.12.12 and
Rust 1.99.1.

Limitations: this is one repeated fixture and no native arm. TextEdit and the
desktop are shared; the initial Find field can retain an earlier search.
Generic plugin/tool catalog notices varied without task coaching. No candidate
attempt naturally triggered input-error recovery; the owned behavior test,
not these successful tasks, validates that branch. These results do not
establish a causal reliability or efficiency improvement.

## Validation and release status

- Source gate: formatting, Clippy, **45 Rust unit tests**, version/skill metadata
  and release build passed through `scripts/verify.sh --skip-desktop`.
- Python tooling: **40 tests passed**, including 34 benchmark-reporting tests.
- Focus and error-ref behavior: old behavior fails the respective reproduction;
  the repaired candidate passes. The background action suite also retains its
  Unicode, target identity, notification and complete-snapshot checks.
- Final full `verify.sh`: **469 passed, 224 failed, 34 skipped / 727** in L1;
  all source checks passed. It did not satisfy the gate. These counts match
  the earlier full attempt; neither run is a passing release gate.

Many broad-suite failures arise from English application-name expectations
(`Finder`, `Calculator`) on this Chinese macOS and missing default preference
keys. Some Broker timing/resource assertions also failed. These must be
classified and resolved rather than dismissing every failure as environmental.
The focused repairs and 3/3 cohort do not waive the full gate or release L2.
Version/tag/PR creation and `skill-stable` promotion have not been performed.

[Machine-readable report](2026-11-03-focus-recovery-fixes.json) contains the
candidate hashes, audits, counters, validation logs' hashes and release status.
Raw evidence remains in ignored `test-results/textedit-fixes-retest-20261103/`.
