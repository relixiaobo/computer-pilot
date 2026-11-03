# Notes title guidance and fresh Luna acceptance — 2026-11-03

**All three tasks passed the existing independent L2 checks with the updated
skill.** This is the user-approved Codex / `gpt-6-luna` alternative, not an
API-backed `run.py` success. The earlier [2/3 Luna cohort](2026-11-03-luna-l2.md)
and its false completion remain unchanged.

## Reproduced behavior and correction

An owned, uniquely named Notes fixture confirmed that replacing `body` without
its title line changes the note's `name`. Restoring a single title heading
restored the exact name and updated text without duplicating the title. The
fixture was deleted by its stable ID and absence was verified.

The skill now routes Notes creation/body edits to a focused scripting reference:
keep the title once in the first line, retain object ID across edits, and read
back both name and plaintext. Completion guidance calls for the requested
output identity alongside content in the final edit group's observation. This
does not add a per-field model round or change CLI behavior, task goals, or
verification expectations.

## Fresh acceptance

Three new, isolated Luna agents received the updated skill and original task
goals, without the previous failure, suggested fix or oracle. They ran serially
using the same public CLI and ordinary shell. Coordinator preparation, frozen
mail source capture, grading and cleanup reused `run.py` functions. The same
25 shell decision calls / 10-minute participant bound was retained; it is not
the API runner's model-round metric. No task was retried after grading.

| Task | Original independent checks | Additional coordinator check | Reported shell calls |
| --- | --- | --- | ---: |
| Mail → Notes | 3/3 passed | Exact requested name; title line appears once | 11 |
| Desktop inventory → Notes | 2/2 passed | Exact name; all 7 current desktop item names and kinds present | 4 |
| Reminder → Calendar | 2/2 passed | Exactly one matching event, tomorrow at 10:00 | 2 |

All three participants reported DONE, all checks passed without skips, and
all fixture cleanup checks passed. The mail agent reported one timed-out read
and recovered with a narrower read; the other participants reported no errors.
Shell counts are self-reported, not independent performance measurements.
The title failure did not recur in this cohort; one cohort is not a causal
reliability or broad model-performance claim. The existing mail oracle verifies
subjects and content length, not every fact in the summary.

## Other validation and release status

- Skill Creator validator: passed.
- `scripts/verify.sh --skip-desktop`: version/metadata, formatting, Clippy,
  **46 Rust tests**, release build passed.
- Python benchmark/runner tests: **39 passed**.
- Owned Notes title/body reproduction and cleanup: passed.
- `git diff --check`: passed.

The full L1 suite was not rerun for this guidance-only change. Existing full-gate
and affected cleanup evidence remains applicable as documented in the preceding
reports. No version bump, commit, release PR, tag or publication was performed.
The automated release script still invokes the API runner; this cohort does not
restore that provider's quota or turn its recorded failure into a pass. Any
release using this approved alternative must accurately identify the Luna
execution route and preserve its evidence.

Private raw records are under `test-results/luna-notes-retest-20261103/` and
`test-results/notes-title-20261103/`. The JSON companion contains sanitized
results and source/evidence SHA-256 digests, without mail or desktop content.


### Release-evidence integration follow-up

The report's source inventory was expanded for the explicit release preflight.
All originally recorded hashes still matched. Every additional CLI, Cargo or
skill file was checked byte-for-byte against `base_commit`; no task, result,
or participant claim was changed. `--agent-evidence` now validates this reviewed
report against the pre-version-bump tree and records its execution route in the
release PR. It does not claim the API provider's quota was restored.

The release integration then passed the full `scripts/verify.sh`: **890 L1
assertions passed, 0 failed, 24 explicit skips**, plus 46 Rust tests and all
metadata/formatting/Clippy/build checks. The hosted Python suite passed **47
checks**, including source-freshness rejection and isolated Git release tests
that create a version branch, push to a local bare origin, and capture a fake
PR request with the external model/route/report recorded. Failed evidence was
rejected before version/branch mutations in real and dry-run executions.
Interactive or unavailable-app checks remain explicit skips, not passes.
