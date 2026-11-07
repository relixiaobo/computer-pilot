# macOSWorld regression dashboard

This dashboard compares recorded results; it does not run macOSWorld.

- Model: `gpt-6.1-sol`
- Cohort: `en/safe_low/sys_apps`; max steps `12`
- Upstream revision: `e2ca8334b3765537e5c1c428ce57990248a1bb5f`

| Date | Run | Binary | Passed | Pass rate | Δ passed | Δ rate |
|---|---:|---|---:|---:|---:|---:|
| 2026-11-07 | 1 | `cu 0.9.6 (ea92af17ae33)` | 1/3 | 33.3% | — | — |
| 2026-11-07 | 2 | `cu 0.9.6 (ea92af17ae33)` | 0/3 | 0.0% | -1 | -33.3 pp |

## Task status changes

| From → to | Task | Result |
|---|---|---|
| 2026-11-07 #1 → 2026-11-07 #2 | `1ef109cd-fea2-d2d5-b4e4-fbbad6689b2a` | fail (fail) → fail (done) |
| 2026-11-07 #1 → 2026-11-07 #2 | `259bcb80-aa2b-8291-1d11-6ff4ae36f8cb` | pass (incomplete) → fail (incomplete) |
