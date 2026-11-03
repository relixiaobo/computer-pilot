# TextEdit final-newline failure analysis — 2026-11-03

This is a replay of retained evidence from `textedit-2-cu` in the
[follow-up resource cohort](2026-11-03-followup-resource-retest.md), not a new
agent run. The historical failure remains a failure.

The saved document differs from the exact oracle by **one extra final LF**.
All requested content changes are present. The final Save action's AX textarea
value equals the independently captured saved text, including that extra LF;
this case does not show a stale snapshot or a save that dropped an edit.

| Retained tool call (zero-based) | Evidence | Final LF count |
| --- | --- | ---: |
| 1 | Initial task document | 1 |
| 10 | Local edits completed; Contacts still at EOF | 1 |
| 11 | Cut from the Contacts heading through EOF; the preceding separator remains | 2 |
| 32 | Other edits completed; last checkbox updated | 2 |
| 33 | Save returns the same document text as the independent saved-file capture | 2 |

The agent repaired other intermediate edits but never reconciled the leftover
source separator with the required final newline. Its completion response claimed
the file was updated, saved and observed, despite the visible format discrepancy.
In the passing `textedit-1-cu` attempt, the moved range included the leading
separator; that is evidence for one working selection, not a mandatory shortcut
sequence for every editor.

## Correction and validation scope

The skill now calls out source/destination separators when moving a section and
literal line-break comparison when the user requires exact whitespace. It reuses
the action group's returned state. It does not add a CLI flag, normalize AX values,
reinterpret a generic Save receipt as verification, or require another observation
round for every edit.

The retained trace was parsed and checked against its own source and saved JSON:
`saved == expected + "\n"`; the initial text has one trailing LF, the cut result
has two, and the final returned textarea equals the saved text. Skill metadata
and local reference links were validated. No fresh LLM cohort was run, so this
change has **no measured agent-success improvement yet**.

Evidence hashes (SHA-256; raw traces remain in the private workspace archive):

- `source.json`: `2d3731edf6b21e558faacc7894395d5ecd2ef4f73ffbbf0e8ccfb3ede7b02936`
- `saved.json`: `a8f8f327beedc5d1c18678cea08b205e8350081fb21862d98ed07929bb990275`
- `trace.json`: `128241f931c3c08473147a973fce210157582b03dea9a3441e311593e62fc9bb`
