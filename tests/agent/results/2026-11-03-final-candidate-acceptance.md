# Independent final candidate acceptance — 2026-11-03

**Accepted for the repair PR/CI stage. Not yet accepted for publication.**
A separate reviewer (`/root/final_acceptance`) reviewed the final working-tree
changes and independently reran critical behavior tests. No new blocking code
finding was identified. The reviewer did not edit source files, commit, push or
publish. Desktop tests ran serially in an assigned interval; the owned focus
fixture restored the original frontmost process.

The candidate is based on `85c29ab` on `fix/retest-focus-and-metrics`, still
reports 0.9.3, and is not the published binary. Binary SHA-256:
`969c7a39379a8f92f3927d2f199f779b60e4c29076e266c2e4b9c636a01f6b6a`.

## Independent validation

| Check | Result |
| --- | --- |
| Rust unit tests | 46 passed |
| Python benchmark tests | 34 passed |
| command_runner, defaults, broker, launch, action_latency, cursor_isolation | 58 passed, 0 failed, 0 skipped |
| Owned focus_activation fixture, explicitly interactive | 1 behavior group passed, 0 failed, 0 skipped |
| Diff whitespace check | passed |
| Prior full-gate evidence | 888 passed, 0 failed, 25 skipped, verified against raw log |

The focus group includes six alternating exact-PID activations, independent
fresh-process frontmost observations, unchanged pointer position, invalid-window
rejection and original-frontmost restoration. It supplements the default gate;
it does not rewrite the historical 25-skip count. The 58 targeted assertions are
a recheck of selected behavior, not an additional count to add to the 888 total.

Before this documentation update, both reviewer and parent verified every entry
in the previous evidence manifest: 57 source hashes, six log hashes, candidate
binary hash and Markdown report hash matched. No production or test source was
changed during this acceptance round. The earlier full gate remains evidence
for this same code candidate. Documentation records below are a subsequent update.

## Review findings

- Launch readiness retains both bundle ID and installation path, rejects a
  different installation sharing the ID, and preserves ambiguous-target errors.
  Ordinary action selectors retain their existing semantics.
- Native activation advances the run loop within the existing deadline. It does
  not introduce global input or an implicit activation path.
- Both input-error recovery paths publish only successful snapshots. The owned
  behavior test directly reuses returned refs and checks the intended control.
- Broker tests verify admission, actual contention, a common PID resource and
  non-overlapping dispatch, instead of inferring lock failures from missing records.
- Suite accounting rejects the reproduced numeric-diagnostic false green. The
  delayed launch fixture's failure cleanup runs through `finally`.
- These changes do not alter private Broker wire/interpretation semantics; no
  protocol bump is needed.

## Disposition of the 25 default-gate skips

| Category | Count | Disposition |
| --- | ---: | --- |
| Interactive tests | 12 | Owned exact-PID focus independently passed now. Five Finder focus assertions overlap that implementation. Electron foreground paste and five selectorless/global-input assertions remain unrun. |
| Preserve an existing TextEdit session | 7 | The user's session was not closed to eliminate skips. Owned action_latency covers relevant input/recovery behavior; this is not a substitute for final-candidate L2. |
| Unavailable app state | 4 | Protected capture/state/WeChat account for three, Safari for one. No corresponding production capture/menu change in this repair. |
| Finder control shape | 2 | No suitable textfield or action-less element in that desktop state. Owned fixtures cover the core input paths. |

The Electron fixture's two default executable locations are absent on this
machine; no Electron foreground result is claimed. No blanket interactive/global
suite was enabled against the shared desktop.

## Release prerequisite still unmet

The parent invoked `python3 tests/agent/run.py` against the final candidate.
It exited **1 at credential preflight**, before touching a task fixture or making
an LLM call. There is no project `.env`; neither API provider key nor `AGENT_MODEL`
is configured in the process environment. The default model therefore reports
its missing Anthropic key. This is **not an L2 behavioral failure or an L2 pass**.

Before a release, configure a supported API credential and matching `AGENT_MODEL`
in the local `.env` (plus `OPENAI_BASE_URL` for a relay), then run the required L2.
The existing Codex-based 3/3 cohort belongs to an earlier candidate and was not
replaced by this preflight. Repair PR/hosted CI may proceed; publication remains
subject to final L2 and the protected release workflow. No release was prepared.

Raw local logs and their SHA-256 digests are indexed in the JSON companion.
