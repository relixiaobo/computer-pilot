# Clean fix retest — 2026-11-02

This run tests the CLI and skill changes against fresh Finder, TextEdit and VS
Code fixtures. Every participant received a fresh profile, thread, cwd and cu
runtime. Project documents and memories were disabled, the clipboard was
isolated, and the shared application process was kept alive with a dedicated
task window. The final state was checked independently from the participant
claim.

The complete sanitized measurements are in
[2026-11-02-fix-retest.json](2026-11-02-fix-retest.json). Raw rollouts, profiles,
clipboard payloads and document contents remain private.

| Case | Backend | Result | Full turn s | Outer calls | Backend calls* | Errors |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| Finder rename | cu | Failed, correctly blocked | 71.9 | 13 | 12 | 2 |
| Finder rename | Native | Pass | 264.1 | 36 | 36 | 1 |
| Finder organize | cu | Pass | 576.3 | 68 | 96 | 2 |
| Finder organize | Native | Pass | 210.3 | 40 | 40 | 0 |
| TextEdit holdout | cu | Pass | 139.7 | 30 | 37 | 0 |
| TextEdit holdout | Native | Failed exact oracle | 123.6 | 15 | 15 | 3 |
| VS Code JSON | cu | Pass | 157.6 | 31 | 27 | 2 |
| VS Code JSON | Native | Pass | 22.5 | 2 | 2 | 0 |
| VS Code Markdown | cu | Pass | 130.5 | 23 | 43 | 2 |
| VS Code Markdown | Native | Pass | 38.8 | 3 | 3 | 0 |
| VS Code multi-file | cu | Pass | 209.5 | 31 | 44 | 3 |
| VS Code multi-file | Native | Pass | 88.8 | 10 | 10 | 1 |
| VS Code Markdown follow-up | cu | Pass | 122.9 | 19 | 39 | 0 |
| VS Code Markdown follow-up | Native | Pass | 128.0 | 15 | 15 | 4 |

`Backend calls` counts actual `./pilot` invocations or native CUA items from
the retained trace. A shell batch can stop before later lines, so this is a
diagnostic action count. Timing and primary comparison continue to use outer
participant tool calls and the full participant turn.

The run has 12 passes out of 14 attempts. Computer Pilot passed 6 of 7 attempts;
the failed Finder rename was a correctly reported model block after invented
`--count` and `find --text` options, with no file mutation. Native passed 5 of 6;
the failed TextEdit run claimed completion but saved one extra final newline.
The exact oracle caught that silent formatting defect.

The only successful Finder pair was organize: cu took 576.3 seconds and native
210.3 seconds. The cu run passed after 76 model calls and 96 estimated backend
dispatches; the main cost was repeated model recovery, not CLI execution. The
Finder rename failure prevents a general Finder speed claim.

VS Code's three cases all passed. Native was faster in the first matrix block,
with a median of 38.8 seconds versus 157.6 seconds for cu. A follow-up Markdown
pair after the leading-hyphen input fix passed on both sides at 122.9 seconds
cu and 128.0 seconds native. This is a single follow-up pair and should not be
treated as an overall performance estimate.

## Changes validated

- A click on a textfield-shaped AX node now reports focus state without failing
  the click. Finder filename fields can represent selection or navigation.
  `cu type` still requires a focused AX text input immediately before sending
  text.
- AX perform failures now preserve the numeric AX error and distinguish a
  rejected action from an action absent from the element's advertised actions.
  The Finder trace recorded this distinction for the rejected AXPress.
- Arrow key aliases such as `arrowdown` are accepted.
- Leading-hyphen text values are accepted by `cu type`; the explicit `--`
  separator remains supported. The follow-up VS Code Markdown run had zero
  syntax errors and passed exact verification.
- The skill now defines one observed UI state per action group, requires a new
  observation across navigation/focus/selection boundaries, and lists the
  actual `click` and `find` predicates to prevent invented options.

## Remaining findings

The native arm still produced silent extra whitespace in TextEdit and had
invalid `CMD`/paste arguments. Native API success does not establish exact
document bytes. Both arms therefore need an independent final artifact check.
Computer Pilot's remaining Finder cost is model recovery and target planning;
reducing CLI latency alone will not address it.

Before making a broad speed claim, run at least three counterbalanced repetitions
per case. Keep cross-app transfer, Electron selection drift and stale-target
fixtures in the matrix. Preserve exact oracles and the distinction between
outer tool calls, backend dispatches, tool time and residual model/network time.
