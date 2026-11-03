# Local gate repairs — 2026-11-03

**The default local `scripts/verify.sh` gate now passes: 888 L1 assertions
passed, zero failed, 25 explicitly skipped.** Formatting, Clippy, all
46 Rust unit tests, version/skill metadata and release build also pass.
The hosted benchmark-reporting check separately passes all 34 Python tests.
This is an unreleased working tree based on `85c29ab`, not a new published release.

## What changed

- **Localized target selection in L1.** Native CLI calls now use stable Finder
  and TextEdit bundle identifiers. Display-name assertions use the identity
  returned by `apps`; menu and OCR checks use actual localized labels. AppleScript
  application terminology remains unchanged. The Swift cursor-motion fixture's
  internal selector is corrected too. Ordinary action selector semantics remain
  unchanged: unique running name, bundle ID or PID, with ambiguity rejected.
- **Launch readiness now follows the opened bundle.** Launch Services resolves
  installed filename aliases before opening the exact bundle path with `open -g`.
  Readiness retains both bundle ID and path, so a different localized display
  name cannot cause a false timeout, and another installation with the same ID
  cannot supply the wrong window. Multiple matching processes still fail as
  ambiguous. No new command, output field, transport or runtime dependency was
  introduced; no private Broker protocol change is needed.
- **Defaults tests own their preferences.** A unique disposable domain replaces
  assumptions that user Dock/Finder keys exist. Boolean, integer and string writes
  are read back exactly, and cleanup deletes only that test domain. Tests no
  longer change the user's Dock preference.
- **Broker assertions prove admission and contention.** The test uses the actual
  localized name and bundle ID, checks both subprocess errors and both request
  records, verifies the common PID resource and overlapping admission interval,
  then proves dispatch does not overlap. A deliberately absent window exercises
  the mutation lock without moving user windows. The original failures were
  commands rejected before Broker admission, not evidence of a lock defect.
- **Command-suite accounting cannot report the observed false green.** The runner
  accepts only one anchored, internally consistent Results summary and treats
  missing/duplicate/inconsistent summaries or a nonzero exit as failures. Previously,
  `PilotLaunchFixture7974 failed` contaminated the numeric failure count and caused
  arithmetic errors followed by a zero-failure total. An isolated runner test now
  covers that diagnostic, summary failures and continued execution of later suites.

## Reproduction and validation

The preceding full gate recorded **469 passed, 224 failed, 34 skipped / 727**.
Many failures shared the same unresolved English app selector, and some suites
stopped before reaching their later assertions. Counts are not directly comparable:
more tests now execute and some launch/lock checks are grouped behavior assertions.
No failure was converted to an unconditional skip to make this gate pass.

The owned launch fixture has a localized name different from its installed filename.
The official v0.9.3 binary opens it successfully but then times out after eight
seconds when waiting by filename. The repaired candidate passes cold/warm launches
by filename, path and bundle ID, no-wait and human output, exact PID/window checks,
foreground/pointer preservation, and rejection of a second installation sharing
its bundle ID. The second fixture has delayed startup; cleanup of both bundles
and their registrations runs even on assertion failure. The fixture lives in an
owned temporary folder under `~/Applications`, where filename lookup is available,
and removes that folder on exit. This replaces repeated launches/quits of Calculator.

The old suite runner fails the new isolated regression; the repaired runner passes.
Independent read-only review found the missing path check and numeric-counter bug;
both are corrected, along with the delayed-fixture failure cleanup it flagged.
The official installed binary remains unchanged (hash in the JSON companion).

Final validation:

- `bash scripts/verify.sh`: **passed**, including **46 Rust tests** and
  **888/0/25 L1 pass/fail/skip**, across 63 command suites.
- `python3 -m unittest discover -s tests/agent -p 'test_benchmark*.py'`:
  **34 passed**.
- `git diff --check`: passed.

Local environment: macOS 26.5.1, Chinese UI; Rust 1.99.1 and Python 3.12.12.
The isolated local toolchain directories were put on PATH for verification.
One intermediate standalone L1 run omitted Cargo from PATH and failed the release
recovery fixture; the complete gate with its required toolchain passes that test.

The 25 skips remain explicit: interactive foreground/global-input tests are
opt-in; the user's existing TextEdit session is preserved; some app/state-dependent
fixtures are unavailable. This is a green **default local gate**, not proof of every
interactive scenario or a new full L2 release run. The earlier 3/3 agent cohort
belongs to the prior candidate and was not rerun for these launch/test changes.
Release-time L2, hosted CI and the protected release workflow still apply.
No commit, push, tag, release PR, publication or skill-stable promotion was performed.

Local raw logs and SHA-256 evidence are indexed in the JSON companion. Earlier
reports retain their historical red-gate results.
