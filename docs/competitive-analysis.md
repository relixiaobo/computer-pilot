# Computer Pilot — Competitive Analysis

> ⚠️ **Stale snapshot — frozen at 2026-04-03.** Predates Sprint 2, the R1–R7 reliability batch, the ScreenCaptureKit migration, capture-protected detection (`screenshot_error`), `cu state` / `cu find` / `cu launch` / `cu warm` / `cu why` / `cu set-value` / `cu perform` / `cu nearest` / `cu observe-region` / `cu menu` / `cu defaults`, the verify-by-default click pipeline, and the `*-pid` method audit field. For current state see [`ROADMAP.md`](./ROADMAP.md). Kept for historical context only.
>
> Update legend: ✅ = shipped, ❌ = not implemented, ⭕ = added this round.

## Perception

| Capability | cu (ours) | Anthropic Computer Use | Ghost OS | agent-desktop | usecomputer | axcli |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| AX tree snapshot | ✅ | ❌ | ✅ | ✅ | ❌ | ✅ |
| Batch AX attribute reads | ⭕ | — | ✅ | ✅ | — | ✅ |
| Per-element timeout | ⭕ 3s | — | ✅ 3s | ✅ 2s | — | ✅ |
| Numbered refs | ✅ | ❌ | ❌ | ✅ | ❌ | ❌ |
| Persistent refs | ❌ | — | ❌ | ✅ | — | — |
| Screenshot | ✅ ScreenCaptureKit (cross-Space) + CGWindowList fallback | ✅ | ✅ | ❌ | ✅ | ✅ |
| Capture without activation | ✅ | ✅ | ✅ | — | ✅ | ✅ |
| OCR | ⭕ Vision (objc2) | ❌ | ❌ | ❌ | ❌ | ✅ |
| VLM fallback | N/A (agent-supplied) | ✅ built-in | ✅ ShowUI-2B | ❌ | ❌ | ❌ |
| Chrome CDP | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| Coordinate offsets | ✅ | — | ❌ | ❌ | ✅ | ❌ |
| Auto-snapshot after action | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |

## Action

| Capability | cu (ours) | Anthropic Computer Use | Ghost OS | agent-desktop | usecomputer | axcli |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Left click | ✅ AX-first | ✅ | ✅ | ✅ 15-step | ✅ | ✅ |
| Right click | ✅ | ✅ | ❓ | ✅ | ✅ | ❌ |
| Double click | ⭕ | ✅ | ❓ | ✅ | ✅ | ✅ |
| AX click chain | ⭕ 14-step | — | basic | **15-step** | — | basic |
| Keyboard shortcuts | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Layout safety | ✅ `--app` | ✅ | ✅ | ❓ | ✅ | ❓ |
| Text input | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Scroll | ⭕ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Drag | ⭕ | ✅ | ❓ | ✅ | ✅ | ❌ |
| Hover | ⭕ | ✅ | ❓ | ✅ | ✅ | ✅ |
| Hold key / modifiers | ⭕ `--shift`/`--cmd`/`--alt` | ✅ | ❌ | ❌ | ✅ | ❌ |
| Wait conditions | ⭕ `--text`/`--ref`/`--gone` | ✅ | ❌ | ✅ | ❌ | ✅ |
| Clipboard | ✅ pbcopy/pbpaste | ❌ | ❌ | ✅ | ❌ | ❌ |

## Engineering

| Capability | cu (ours) | Anthropic Computer Use | Ghost OS | agent-desktop | usecomputer | axcli |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Architecture | Single-binary CLI | API tool | MCP daemon | Multi-crate CLI | N-API CLI | CLI |
| Language | Rust | Python | Swift | Rust | Zig + TS | Rust |
| Binary size | 1.2 MB | — | ~50 MB + 3 GB | <15 MB | ~5 MB | ~3 MB |
| Dependencies | zero | Python | macOS | zero | Node.js | zero |
| Latency | <10 ms | 3–8 s | <100 ms | ~200 ms | ~100 ms | ~100 ms |
| JSON / human output | ✅ auto | JSON | JSON | JSON | JSON | mixed |
| Permission onboarding | ✅ dual check | ❌ | docs only | ❌ | ❌ | ❌ |
| Token efficiency | ✅ text-first | ❌ ~1400/screenshot | ✅ | ✅ | ❌ | ✅ |

---

*Last updated: 2026-04-03 — Added: scroll, double-click, hover, drag, wait, modifiers, OCR, batch AX, 14-step click chain, per-element timeout, clipboard*
