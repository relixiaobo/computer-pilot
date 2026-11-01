# Clean real-application pilot — 2026-11-02

Four fresh participant runs passed independent artifact verification. This is
one pair per task, with cu first in both pairs. It is not a repeated estimate of
overall success rate or speed. The CLI and packaged skill stayed frozen.

[Sanitized measurements and evidence hashes](2026-11-02-clean-pilot.json)
include model usage, instruction audits, versions and fingerprints. Raw rollouts,
profiles and clipboard backups remain private.

| Task | Backend | Exact artifact | Full turn s | Model calls | Outer tool calls | Tool errors | Tool interval s | Remaining interval s |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Finder organization | cu | Pass | 253.811 | 34 | 33 | 4 | 12.461 | 241.350 |
| Finder organization | Native | Pass | 344.145 | 62 | 61 | 2 | 37.395 | 306.750 |
| TextEdit maintenance handoff | cu | Pass | 189.860 | 19 | 18 | 2 | 7.793 | 182.067 |
| TextEdit maintenance handoff | Native | Pass | 154.833 | 17 | 16 | 1 | 12.574 | 142.259 |

Finder cu/native elapsed ratio was 0.738; TextEdit was 1.226. These are individual
paired observations, not stable speed claims. All four completion claims agreed
with the saved artifacts. Errors and recovery are retained in the totals.

## What was controlled

Both arms used `gpt-6-astra`, high reasoning effort, and the same host. The
recorded service tier was null. Every run had a new profile, thread, neutral
working directory, task data and cu runtime. Project document injection and
memories were disabled; actual instruction records showed no project AGENTS
injection. Participants received their assigned public tool documentation.
All tool arguments were reviewed for backend compliance and prohibited file IO.

Both tasks were UI-only. Each turn included skill/API discovery and all recovery.
Coordinator setup, clipboard isolation, passive verification and cleanup were
outside the timer. Finder and TextEdit shared the user's application processes,
with dedicated task windows. Application preferences and desktop state were not
reset; this is not a fresh macOS session. All four task windows were closed.

Finder's oracle checked the complete file set and contents, including removal
from the source directory. TextEdit's oracle checked exact saved bytes, including
unchanged archived values, Unicode, section spacing and the final newline.
Participants could not read the expected artifacts or receive verifier feedback.

## Findings

- **Isolation changed the apparent result.** An earlier Finder attempt pasted
  old clipboard files after unsupported Cmd-X, then removed its originals. Its
  final bytes matched, but the workflow depended on prior data. All four earlier
  cohort attempts are excluded, including rename attempts where dependency was
  not observed. The new trials save all pasteboard item types privately, clear
  them before starting, and conditionally restore them without overwriting new
  user content. Prior exclusion reasons remain in the JSON artifact.
- **Recovery dominates these runs.** Both Finder agents attempted unsupported
  Cut. Native also repeated rename/menu recovery; cu encountered focus and AX
  action errors. The much smaller tool intervals do not explain the whole-turn
  difference. Remaining time includes model, network and orchestration work.
- **Precise text selection is a useful capability to investigate.** Native used
  contextual `selectText`; cu used Find and keyboard navigation. cu recovered
  from an unsupported key spelling, an unchanged search write, and an insertion
  that replaced the selected archived heading. Native also had a failed grouped
  selection and checklist character/newline corruption, which it repaired after
  observing state. Selection capability alone did not guarantee correctness.
- **Successful dispatch is insufficient.** Native checklist input returned
  without a tool error while the text was wrong. Both arms needed observations
  to detect and repair mistakes. Removing those checks would hide accuracy loss.
- **Post-save counts need interpretation.** Finder had 3 cu / 4 native calls
  ending after the first exact file observation; TextEdit had 1 / 1. These counts
  include an overlapping save call and task-required final inspection, so they
  are not counts of unnecessary calls.

## Follow-up supported by the evidence

Independently reproduce cu's classification of Finder filename text fields:
selection/navigation should not be judged solely by whether an editable text
field gained focus. Also distinguish an AX action's dispatch failure or unknown
outcome from an unsupported action; the observed AXOpen error listed AXOpen as
available. Preserve target identity and exact-effect checks while addressing
these semantics. Investigate selection and caret behavior across multiple native
and Electron editors before choosing a general interface change.

Run the planned repetitions with randomized order before making performance
claims. Cross-app transfer and the new stale-target fixture remain unimplemented;
this pilot does not claim six-class coverage or duplicate-label disambiguation.

## Harness verification

The 16 offline real-app benchmark tests passed, including copy-versus-move,
extra-file/symlink rejection, cleanup ownership and reproducible paired ordering.
The runner compiled and executed both TextEdit arms. Clipboard payload round-trip
and preserving a subsequently changed clipboard were checked during setup work.
After the frozen trials, a script-serialization fix removed sequential placeholder
substitution from clipboard restoration; a JXA smoke check preserved literal
`ROOT`/`SAVED` data without touching the system clipboard. Both the frozen and
post-run harness hashes are retained. No CLI or skill behavior was changed.
