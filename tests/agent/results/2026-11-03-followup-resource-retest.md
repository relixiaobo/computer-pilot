# Follow-up resource-based long-task retest — 2026-11-03

This cohort ran after the observation/focus fixes at commit `006f894`. Primary measures are exact saved artifacts, model rounds and tokens. Each attempt had 100 model responses and 8,000,000 cumulative tokens; wall time is diagnostic only. Native Computer Use was not rerun. Applications and desktop were shared with dedicated test windows.

**10/11 normal attempts passed exactly; 1 normal failure; 1 budget stop; 11/12 independent artifact checks were exact.**

| Attempt | Result | Exact artifact | Rounds | Total tokens | Input | Cached input | Output | Reasoning | Tool calls |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| finder-organize-r1-cu | passed | yes | 44 | 2477595 | 2470996 | 2307072 | 6599 | 3997 | 42 |
| finder-organize-r2-cu | passed | yes | 50 | 2575549 | 2570290 | 2230656 | 5259 | 2358 | 44 |
| finder-rename-r1-cu | passed | yes | 16 | 583164 | 580343 | 441216 | 2821 | 1598 | 14 |
| finder-rename-r2-cu | passed | yes | 46 | 2426043 | 2419299 | 2164096 | 6744 | 3953 | 41 |
| textedit-1-cu | passed | yes | 11 | 379203 | 377086 | 298496 | 2117 | 1026 | 10 |
| textedit-2-cu | failed | no | 35 | 1409186 | 1396962 | 1303168 | 12224 | 7088 | 34 |
| vscode-json-r1-cu | passed | yes | 12 | 395466 | 392218 | 326144 | 3248 | 1998 | 11 |
| vscode-json-r2-cu | passed | yes | 9 | 291250 | 288518 | 255616 | 2732 | 1390 | 8 |
| vscode-markdown-r1-cu | passed | yes | 7 | 198599 | 196003 | 153600 | 2596 | 1512 | 6 |
| vscode-markdown-r2-cu | passed | yes | 7 | 203596 | 201361 | 174208 | 2235 | 1289 | 6 |
| vscode-multi-r1-cu | budget_stopped | yes | 74 | 8010871 | 7994091 | 6765312 | 16780 | 7765 | 68 |
| vscode-multi-r2-cu | passed | yes | 33 | 2181565 | 2176704 | 2015488 | 4861 | 2113 | 32 |

Totals: **344 rounds / 21132087 tokens**; input 21063871 (cached 18435072, uncached 2628799), output 68216 (reasoning 36087).

Findings:

- Finder completed all four tasks exactly. Organize-r2: 50 rounds / 2,575,549 tokens; rename-r1: 16 / 583,164. The earlier cohort used 94 / 7,284,117 and 33 / 1,592,491 respectively, but these are separate fresh runs and do not prove a causal improvement.
- TextEdit remained variable: one exact pass used 11 / 379,203; the other used 35 / 1,409,186 and failed the saved-byte check. That failed attempt still claimed completion, so this cohort has 1 false completion claim among 11 claims. Focus and settle fixes do not by themselves prove committed document state.
- VS Code Markdown passed in 7 / 198,599 and 7 / 203,596. JSON passed in 9 / 291,250 and 12 / 395,466. Multi-r2 passed in 33 / 2,181,565. Multi-r1 reached the token budget at 74 / 8,010,871; its independent file samples were exact, but it had no normal final response and is retained as a budget stop.
- No out-of-scope window violation was recorded in this cohort. Shared application processes and desktop remain a limitation.

Protocol: fresh profiles, threads, neutral cwd, fixtures and cu runtimes; project docs/memories disabled; no AGENTS injection; same gpt-6-astra/high/null-tier settings. Every attempt is retained. Cached input is part of input, reasoning is part of output, and neither is double-counted. A model round is one response ID; tool calls are separate diagnostics.

Machine-readable data: [JSON report](2026-11-03-followup-resource-retest.json).
