# Fresh TextEdit section-boundary retest — 2026-11-03

Three predeclared, fresh-context cu-only attempts tested the post-release skill
at `85c29ab` with the official signed/notarized **v0.9.3 CLI**. The existing
TextEdit holdout fixture was unchanged. All attempts were retained and audited
independently after completion; no participant received oracle feedback.

**2/3 tasks passed exactly; one ended honestly blocked by `focus_failed`.
There were no false completion claims among two claims, with all three final
responses audited.** This provides two successful fresh exercises of the
section-boundary guidance, not proof of a causal reliability improvement.

| Attempt | Result | Saved bytes exact | Completion claim | Model rounds | Total tokens | Shell / image-view calls |
| --- | --- | --- | --- | ---: | ---: | ---: |
| textedit-1-cu | Failed: focus blocked | no; original file unchanged | no | 6 | 155,481 | 5 / 0 |
| textedit-2-cu | Passed | yes | yes | 12 | 383,420 | 11 / 0 |
| textedit-3-cu | Passed | yes | yes | 22 | 778,580 | 15 / 6 |

Totals: **40 model rounds, 1,317,481 tokens and 37 outer tool calls**. Input
was 1,313,408 tokens, including 1,236,352 cached tokens; output was 4,073,
including 738 reasoning tokens. Cached input and reasoning are subsets, not
additional tokens in the total. The existing collector reports 31 tool calls
because it omits six image-view calls in attempt 3; the table separates them.
Four shell calls returned errors. No attempt exceeded its resource budget.

## Independent evidence

- Attempt 1 read the correct task document, then `window focus` returned
  `focus_failed`: requested PID 72327, reported frontmost PID 3388
  (`UserNotificationCenter`). `set -e` stopped the remaining input group. A
  follow-up target observation showed `frontmost:false` and unchanged content.
  The final response reported blocked. The 463 saved bytes equal the original
  fixture; no evidence establishes the cause of the activation failure.
- Attempt 2 used scoped Find matches and UI editing. After one paste returned
  `unknown_outcome`, it observed unchanged content, explicitly refocused the
  test window, and recovered. Its Contacts selection included the leading
  separator; the cut result retained one final LF. Final AX text and disk bytes
  equal the oracle.
- Attempt 3 corrected an initially incomplete Contacts search after inspecting
  screenshots. Its successful cut included the preceding separator and full
  section. Destination typing returned `unknown_outcome`; one subsequent AX
  write was rejected as stale without dispatch, then recovered using the new
  diagnostic observation. Final AX text and saved bytes equal the oracle.

Both passing files contain **466 UTF-8 bytes, exactly one final LF**, and have
SHA-256 `b334b3427d16d7a56a3166403b1831b284b6b8bddecccad1ed73d83cdaa3a9aa`.
Independent 250-ms sampling observed exact saves before the final completion
responses and retained exact final samples. Scoped changes, section order,
Archived content, tickets, Unicode, checkbox states and blank lines were all
checked. No forbidden scripting, direct document access, whole-document
reconstruction, foreign-window edits or external messages were found.

The historical extra-LF failure remains a failure. This cohort does not replace
it or demonstrate that the failure class has been eliminated.

## Protocol and environment

- Three serial attempts; fresh profiles, threads, neutral working directories,
  fixtures, client keys, Broker runtimes and output directories. Actual contexts
  match `gpt-6-astra / high / null tier`; no AGENTS injection was detected.
- Each attempt had 100 model responses and 8,000,000 cumulative tokens, with
  no task wall-time deadline. All finished normally, including the blocked
  response. No replacement, mid-cohort fix or participant coaching occurred.
- macOS 26.5.1 (25F80), TextEdit 1.20 with Chinese UI, Codex CLI
  0.166.0-alpha.5.1, project-local Python 3.12.12. The released CLI was installed
  with the packaged installer and used through a `target/release/cu` symlink;
  it was **not built locally**. The skill and relevant inputs were hash-frozen.
- Accessibility and Screen Recording were granted before participants started;
  an owned preflight document confirmed AX reading and capture. A missing
  initial permission and Python 3.9 incompatibility were untimed setup issues.
- TextEdit and the desktop were shared. Developer contexts differed in plugin
  catalog availability, without task coaching. An independent read-only audit
  covered all three complete traces, final responses and saved artifacts.
- Test windows were closed and their private Brokers stopped. Clipboard
  restoration recorded `skipped_changed` for all three attempts; preservation
  of the original clipboard is not claimed.

## Release decision

**Do not declare a new release ready from this cohort alone.** Record the two
exact skill-level successes, retain and investigate the focus-blocked attempt,
and complete the normal release gates before preparing a release. The source
release gate (`verify.sh`), full L1 suite and release L2 suite were not run;
this machine has no Rust toolchain or VS Code. Version synchronization and
all **32 benchmark-tooling unit tests passed**.

No release version, tag, PR or `skill-stable` promotion was created. A
documentation/skill follow-up does not by itself require a new CLI release.

Machine-readable evidence and hashes:
[JSON report](2026-11-03-textedit-boundary-retest.json). Raw local traces remain
under the ignored `test-results/textedit-boundary-retest-20261103/` directory.
