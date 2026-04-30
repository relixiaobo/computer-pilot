# Computer Pilot — Codex CU Alignment Roadmap

> **Goal**: bring `cu` to parity with Codex Computer Use on macOS native apps for the "low-disruption + high-success-rate" experience.
> **Scope**: the model layer is not our battlefield. This roadmap covers only the tool layer (perception / action / loop / robustness / infra / DX).
> **Tracking**: tick each item as it ships, attach the PR link, and fill the actual merge date under "Completed".
> **Companion docs**:
> - [`competitive-analysis.md`](./competitive-analysis.md) — feature grid across projects (factual snapshot)
> - This doc — execution plan + progress log (live)

Last updated: 2026-04-28 (**Sprint 1 + Sprint 2 complete · v0.5.x released**; Sprint 3 in progress — A2 axPath / D8 AX warmup / B7 cu why done; the four root-cause fixes from the E3 WeChat task gap analysis are done (cu key terminal safety check / cu state combined command / cu type --paste / cu click --verify); A5 dropped; A capture-protected / B SCK / R1–R7 reliability batch landed in v0.5.2 — see "Reliability batch" below; E1 / E2 pending; **27 commands, 700+ test assertions**)

---

## Reliability batch (2026-04-28, v0.5.2)

After the E3 wrap-up the user asked for a systematic review of "what information is wrong." We landed 7 root-cause fixes plus 2 underlying migrations in one batch and lifted agent reliability to a new tier:

| ID | Fix | Class of "wrong information" it kills |
|---|---|---|
| A | `cu screenshot` detects capture-protected windows (`kCGWindowSharingState=0`) | No more blank PNGs for WeChat-class privacy windows — returns a structured error instead |
| B | Window-screenshot primary path migrated to ScreenCaptureKit | Cross-Mission-Control-Space windows can be captured (CGWindowList returns empty when on another Space) |
| R1 | `screenshot::find_window` goes through AX (`AXFocusedWindow` + `_AXUIElementGetWindow`) | snapshot/screenshot/click resolve to the same authoritative window — no more "look at A, capture B" |
| R2 | `cu click` verify enabled by default | Sandboxed apps silently swallow PID-targeted events — `ok=true` but the UI didn't change |
| R3 | `cu snapshot` attaches `truncation_hint` string when truncated | Replaces the implicit `truncated:true` boolean — agents skip booleans, not strings |
| R4 | `annotate_window` also goes through SCK | Annotated screenshots no longer come back blank for off-Space windows |
| R5 | Element title walks AXTitle → AXDescription → AXHelp → AXIdentifier | Electron/CEF apps store internal IDs in AXTitle; the user-facing label lives in AXHelp |
| R6 | `cu ocr` attaches aggregate confidence fields + `confidence_hint` | Vision returns plausible-looking hallucinations in the 0.2–0.4 range |
| R7 | `cu type` auto-routes through paste for CJK / chat apps | WeChat/Slack/Discord/Telegram/Lark/QQ/DingTalk drop the leading character |

**Anti-pattern record**: CLAUDE.md / AGENTS.md gained an "Agent Reliability Principles" section — three principles (single source of truth · loud failures · confidence-tiered output) plus an anti-pattern checklist, each pinned to a specific commit. The first thing a new agent reads when joining the project.

**Regression tests**: R3–R7 each have dedicated `tests/commands/test_*.sh` (test_truncation / test_label_fallback / test_ocr_confidence / test_paste_auto) guarding them. Command-test count moved 673 → 699. R4's cross-Space verification is manual only.

---

## Current gap snapshot (vs Codex CU)

| Dimension | cu state | Codex CU | Gap |
|---|---|---|---|
| Model | decoupled, external agent picks | GPT-5.4 built-in | not our battlefield |
| AX-tree-first | ✅ | ✅ | none |
| AX action chain | ✅ 15-step (`src/ax.rs:553`), **finer than open-source peers** | inferred similar | none (slight edge) |
| Action without focus theft | ✅ PID-targeted (B1+B5) | ✅ PID-targeted | aligned |
| Keyboard without clipboard pollution | ✅ Unicode CGEvent (B2) | ✅ unicode + postToPid | aligned |
| `set-value` / `perform` as first-class commands | ✅ B3 + B4 | ✅ | aligned (`find` left to Sprint 2 A1) |
| Closed-loop error hints | ✅ structured CuError (C2) | ✅ structured | aligned |
| Focus / modal summary | ✅ A4 + A6 | partial | aligned (A6 unique to cu) |
| Path-audit field | ✅ `method` field with subdivisions (F2a) | ✗ | **cu only** |
| Multi-monitor | 🟡 single-display OK | ✅ | medium (Sprint 2 D1) |
| Soft-cursor overlay | ❌ | ✅ | won't ship (violates zero-dependency principle) |
| Test infrastructure | ✅ 700+ command-test assertions + agent E2E (runs every release) + macOSWorld 133 tasks | closed-source | cu slight edge |

**Key takeaway**: cu picked the right path. The remaining gap is the "last-mile craft" — Sprint 1 (3 days) covers the most user-felt portion. **Sprint 1 is complete (2026-04-27)**: every "no focus theft / no clipboard pollution / structured errors / focus + modal summary / set-value + perform first-class" gap is closed, and we surpass Codex CU on two axes (method-field audit, modal warning).

---

## Sprint 1 — non-disruptive UX + tool surface (target: 3 days)

Acceptance: every action command leaves the user's real cursor alone, doesn't steal global frontmost, and doesn't pollute the clipboard; `set-value` / `perform` / `find` all ship as first-class commands.

- [x] **B1** PID-targeted CGEvent path in `mouse.rs` (0.5d) — **done 2026-04-27**
  - **Approach**: extend every `mouse::*` signature with `target_pid: Option<i32>`; `Some(pid)` goes through `CGEventPostToPid` + a freshly-built combined-session `CGEventSource` (RAII `EventSource` wrapper); `None` falls back to the global `cghidEventTap`. `cmd_click`'s three modes (OCR / coords / ref) all pass `Some(pid)` when the target pid is known.
  - **Files changed**: `src/mouse.rs` (FFI + EventSource RAII + 5 public signatures), `src/main.rs` (cmd_click 3-mode wiring; cmd_scroll/hover/drag pass None for now — picked up in B5)
  - **References landed**:
    - [iFurySt/open-codex-computer-use `InputSimulation.swift`](https://github.com/iFurySt/open-codex-computer-use/blob/main/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/InputSimulation.swift) — `clickTargeted()` + `.combinedSessionState`
    - [ringotypowriter/kagete `Input.swift:51`](https://github.com/ringotypowriter/kagete/blob/main/Sources/kagete/Input.swift) — `click(toPid:)`
  - **Verified** (`tests/commands/verify_no_disruption.sh`):
    1. ✅ `cu click 5 5 --app Finder` → cursor + frontmost untouched
    2. ✅ `cu click 1 --app Finder` (AX path) → same
    3. ✅ Control: `cu click 7 7` (no `--app`) → cursor warps from (450,81) to (7,975), frontmost stays Ghostty (the global path still works)
  - **Tests**: 258/258 command tests green

- [x] **B2** PID-targeted unicode path for `cu type` / `cu key` (0.5d) — **done 2026-04-27**
  - **Approach**: `key.rs` gains `type_text(text, target_pid)` using `CGEventKeyboardSetUnicodeString` + `CGEventPostToPid`, UTF-16 char-by-char with a 3ms gap; `key::send` adds `target_pid`. `cmd_type` / `cmd_key` use the PID path when `--app` is set. The old `system::type_text` (clipboard paste) and `system::send_key` (AppleScript activate) are deleted — git history retains them as a future sandbox-app fallback reference.
  - **Files changed**: `src/key.rs` (FFI + EventSource RAII + `type_text` + signature extension on `send`), `src/main.rs` (cmd_type/cmd_key wiring), `src/system.rs` (-145 lines of dead code + unused-import cleanup)
  - **References landed**:
    - [iFurySt `InputSimulation.swift:typeText(_:pid:)` / `pressKey(_:pid:)`](https://github.com/iFurySt/open-codex-computer-use/blob/main/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/InputSimulation.swift) — copy-paste-able
    - [kagete `Input.swift`](https://github.com/ringotypowriter/kagete/blob/main/Sources/kagete/Input.swift) — empirical 3ms gap
  - **Verified** (`verify_no_disruption.sh` upgraded with cursor-parking + tolerance):
    1. ✅ `cu type "..." --app Finder` → cursor doesn't drift, clipboard sentinel preserved
    2. ✅ `cu key escape --app Finder` → cursor doesn't drift, frontmost stays Ghostty
    3. ✅ Control: `cu click 7 7` (no `--app`) → cursor warps to (7, 975)
  - **Tests**: 258/258 green

- [x] **B5** PID-targeted scroll / hover / drag (0.3d) — **done 2026-04-27**
  - **Approach**: B1 already widened `mouse::scroll` / `hover` / `drag` to take `target_pid: Option<i32>`; this step only wires the CLI — `Cmd::Scroll` / `Hover` / `Drag` get `--app`, the handler resolves to a pid and passes `Some(pid)`.
  - **Files changed**: `src/main.rs` (3 enum variants + 3 dispatch arms + 3 cmd functions)
  - **References landed**: [iFurySt `InputSimulation.swift:scrollTargeted()` / `dragTargeted()`](https://github.com/iFurySt/open-codex-computer-use/blob/main/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/InputSimulation.swift)
  - **Verified** (3 new entries in `verify_no_disruption.sh`):
    1. ✅ `cu scroll down 1 --x 500 --y 500 --app Finder` → cursor stays
    2. ✅ `cu hover 100 100 --app Finder` → cursor stays (hover's PID path dispatches mouseMoved to the target process only, doesn't move the real cursor)
    3. ✅ `cu drag 100 100 200 200 --app Finder` → cursor stays

- [x] **B3** `cu set-value <ref> "text"` first-class command (0.3d) — **done 2026-04-27**
  - **Approach**: `ax.rs` gains `pub fn ax_set_value(pid, ref_id, limit, value)` — reuses the `find_element_by_ref` walker pattern as `find_and_set_value`, then calls the existing `try_set_value(element, "AXValue", cfstr(value))`. New CLI command `cu set-value <ref> <value> --app X`. On failure returns a structured hint ("try `cu click <ref>` to focus then `cu type` instead").
  - **Files changed**: `src/ax.rs` (+50 lines: `find_and_set_value` + `ax_set_value`), `src/main.rs` (+30 lines: `SetValue` variant, dispatch, `cmd_set_value`), `tests/commands/test_set_value.sh` (+102 lines, 15 assertions)
  - **References landed**: [kagete `set-value`](https://github.com/ringotypowriter/kagete) — the only first-class precedent in the open-source field
  - **Verified**:
    1. ✅ `cu set-value 1 "..." --app TextEdit` writes into the document (AppleScript readback)
    2. ✅ Unicode (Chinese `你好世界`) NFC-normalized comparison passes
    3. ✅ Re-write replaces, doesn't append
    4. ✅ TextEdit doesn't get raised, user cursor doesn't drift, clipboard preserved (in `verify_no_disruption.sh`)
    5. ✅ `ref=0` / nonexistent ref / non-writable element → `{ok:false, error, hint}`
  - **Tests**: 273/273 green (+15 new)

- [x] **B4** `cu perform <ref> <AXAction>` general-purpose command (0.3d) — **done 2026-04-27**
  - **Approach**: `ax.rs` gains `pub fn ax_perform(pid, ref_id, limit, action)`; new `AXUIElementCopyActionNames` FFI + `copy_action_names` helper — on failure we hand the agent the element's **actually supported** actions list. New CLI command `cu perform <ref> <action> --app X`.
  - **Files changed**: `src/ax.rs` (+90 lines: FFI, `copy_action_names`, `find_and_perform_action`, `ax_perform`), `src/main.rs` (`Cmd::Perform` + dispatch + `cmd_perform`), `tests/commands/test_perform.sh` (+85 lines, 17 assertions)
  - **References landed**:
    - [gxcsoccer/axon `perform`](https://github.com/gxcsoccer/axon) — command shape
    - [kagete `action`](https://github.com/ringotypowriter/kagete) — design philosophy
  - **Verified**:
    1. ✅ `cu perform 1 AXShowDefaultUI --app Finder` succeeds, returns `available_actions`
    2. ✅ `cu perform 1 AXBogus --app Finder` returns structured hint + suggested_next (with the element's actual supported actions)
    3. ✅ ref 0 / nonexistent ref / wrong action all flow through C2's structured error path
  - **Tests**: 290/290 green (+17 new)

- [x] **C2** structured failure hints (0.5d) — **done 2026-04-27**
  - **Approach**: new `src/error.rs::CuError { error, hint, suggested_next, diagnostics }` with a fluent builder (`CuError::msg(...).with_hint(...).with_next(...).with_diagnostics(...)`). `From<String>` / `From<&str>` lets existing code stay put — the only mechanical change is upgrading `Result<(), String>` to `Result<(), CuError>` (20 sites, plus 9 `Err(string)` sites adding `.into()`). The `main()` error formatter prints all fields in JSON; human mode prints `Error / Hint / Try` on three lines. `ax_set_value` and `ax_perform` failure paths use the builder.
  - **Files changed**: `src/error.rs` (new, 80 lines), `src/main.rs` (mod ref, 20 signature swaps, 9 `.into()`s, error formatter rewrite), `src/ax.rs` (CuError import, `ax_set_value` / `ax_perform` use builder)
  - **References landed**:
    - [ghostwright/ghost-os `Common/Types.swift:ToolResult`](https://github.com/ghostwright/ghost-os) — `success/error/suggestion` shape
    - Anthropic tool-API error-return philosophy (detailed enough that the model can retry directly)
  - **Verified**:
    1. ✅ `cu set-value 1 "x" --app Finder` failure → `{ok:false, error, hint:"...", suggested_next:[...]}`
    2. ✅ `cu perform 1 AXBogus --app Finder` failure → all fields + `diagnostics.available_actions`
    3. ✅ Tests assert every structured field is propagated
  - **Follow-up**: other `cmd_*` failure paths (cmd_click missing element, cmd_type illegal char, …) still return raw String errors; can be backfilled with hints in Sprint 2. The framework is in place.

- [x] **A4** snapshot top-line "Focused" summary (0.2d) — **done 2026-04-27**
  - **Approach**: snapshot internally reads `AXFocusedUIElement` of the app, extracts role/title/value/position, and reverse-looks-up the ref_id in the already-collected elements list by `(role, x, y)` (1px tolerance). When the focused element is past `--limit`, role/title/value still appear with `ref=None`. New struct `FocusedSummary { ref?, role, title?, value? }` attached to `SnapshotResult.focused`. Human mode prints `Focused: [N] role "title" value="..."` on lines 1–2.
  - **Files changed**: `src/ax.rs` (+30 lines: `FocusedSummary` struct + `detect_focused`), `src/main.rs` (`print_snapshot_human` +9 render lines)
  - **References landed**: [iFurySt `AccessibilitySnapshot.swift:focusedSummary`](https://github.com/iFurySt/open-codex-computer-use/blob/main/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/AccessibilitySnapshot.swift)
  - **Verified**: live test on `cu --human snapshot TextEdit` outputs `Focused: [1] textarea "" value="..."` — focused field + ref + current content

- [x] **A6** snapshot top-line modal warning (0.3d) — **done 2026-04-27**
  - **Approach**: snapshot calls `detect_modal(window_el)` — first checks the window's own AXRole/AXSubrole (AXSheet / AXSystemDialog / AXDialog), then scans direct children for AXSheet. On hit, returns `ModalSummary { role, subrole?, title? }` attached to `SnapshotResult.modal`. Human mode prints a loud `⚠ Modal: AXSheet "..."` line.
  - **Files changed**: `src/ax.rs` (+45 lines: `ModalSummary` + `detect_modal`), `src/main.rs` (`print_snapshot_human` +8 render lines)
  - **Reference**: own implementation (no open-source peer has this)
  - **Verified**: triggered Cmd+W on an unsaved TextEdit document; `⚠ Modal: AXSheet ""` appears at the top of snapshot, agent immediately knows to handle the sheet first
  - **Bonus**: elements inside the sheet automatically become the snapshot body (focused also auto-updates to the sheet's input field), forming a complete "ready-to-act" view

- [~] **F2** `--background` global flag — **closed, not implemented (2026-04-27)**
  - **Reason for closing**: judged redundant after review. B1/B2/B5 already exposed the "PID-targeted or not" toggle at the `--app` level — passing `--app` means non-disruptive, omitting it means global by default; agents are already strongly encouraged to always pass `--app` for reliability reasons, so the "non-disruptive" path is **already the default**. Adding a global `--background` would introduce semantic duplication (`--background` + no `--app` — which path does it take?) and break the "method field is the single source of truth" design.
  - **Alternative**: solve "path transparency" via F2a (subdivide `method`); solve "user/agent knows why" via F2b (document the focus model).
  - **Trade-off**: iFurySt uses an env var because their entire binary is PID-targeted-by-default and needs an opt-in to fall back. cu is a CLI tool that routes by call semantics — a global toggle isn't needed.

- [x] **F2a** subdivide `method` field, distinguish PID vs global path (0.1d) — **done 2026-04-27**
  - **Approach**: every action command's JSON `method` field gets a path suffix. `cmd_click` 3 modes: ref mode is always `ax-action` / `cgevent-pid` / `cgevent-right-pid` / `double-click-pid` (pid known); coord mode chooses between `cgevent-pid` / `cgevent-global` / `double-click-{pid,global}` / `cgevent-right-{pid,global}` based on `--app`; OCR mode chooses `ocr-text-pid` / `ocr-text-global`. `cmd_type` adds `unicode-pid` / `unicode-global`, `cmd_key` adds `key-pid` / `key-global`, `cmd_scroll` / `hover` / `drag` each add `cgevent-pid` / `cgevent-global`.
  - **Files changed**: `src/main.rs` (7 method-field add/rename sites), `tests/commands/test_click.sh` (accept new method names)
  - **Value**: agent can tell at a glance whether the call took the PID-targeted path; any `*-global` is the audit signal for "forgot `--app`, just disrupted the user."
  - **Tests**: 296/296 green (including click suite re-asserted)

- [x] **F2b** document the focus model (0.1d) — **done 2026-04-27**
  - **Approach**:
    - `plugin/skills/computer-pilot/SKILL.md` adds `## Focus Model (why --app matters)` — method-field table + known limitations
    - `CLAUDE.md` rule 6 (was "Key/Type targeting") rewritten as "Focus model — `--app` and PID-targeted delivery", explaining the `EventSource` RAII / `CGEventPostToPid` / `CGEventKeyboardSetUnicodeString` triad — gives future contributors a clean working model
    - `cu type` description synced (no longer clipboard paste, now Unicode CGEvent)
  - **Value**: agent and future contributors share a single doc source for "what `--app` actually guarantees"

- [~] **E3** "no focus theft" automated test — **shipped (2026-04-27)**
  - **Where**: `tests/commands/verify_no_disruption.sh` (11 assertions: cursor location + frontmost app + clipboard sentinel)
  - **Coverage**: B1 (click), B2 (type/key), B3 (set-value), B5 (scroll/hover/drag) — plus a control group showing the cursor really does warp without `--app`
  - **Why not in run_all.sh**: needs ~5s waits + a clean desktop state, would slow the regular suite materially. Stays manual (mentioned in README).
  - **Follow-up**: revisit when D1 multi-monitor lands in Sprint 2 — may be worth promoting then

- [x] **F1** README "Why cu doesn't disrupt your workflow" comparison table (0.3d) — **done 2026-04-27**
  - **Approach**: README adds a `## Why cu doesn't disrupt your workflow` section after `## Why cu?`, with two tables —
    1. cu vs Codex CU vs Anthropic CUA vs kagete on cursor / frontmost / clipboard / IME / perception layer / AX chain / method audit (7 dimensions)
    2. full method-field table (`ax-action` best / `*-pid` non-disruptive / `*-global` disruptive)
    + explainer for the `CGEventPostToPid` + `CGEventKeyboardSetUnicodeString` pair; closes with the inevitable cursor move for drag/hover and the sandbox-app fallback note.
  - **Value**: communicates "where we're behind Codex CU vs where we win" in a shareable single-page table.

---

## Sprint 2 — VLM bridge + CLI craft + closed-loop precision (target: 6 days)

> **Strategic stance**: cu is not turning into a VLM agent. It's the macOS control CLI that already-vision-equipped agents use most smoothly. So Sprint 2 prioritizes **VLM-friendly bridge tools** (A series) + **agent-friendly CLI craft** (G series) above closed-loop precision (B/C/D).
>
> **Acceptance**: ① VLM agents can "look at the image → click by ref" (annotated screenshot + pixel-to-ref reverse lookup); ② agents can pick the right tool from the CLI alone, without reading SKILL.md (per-command selection rules inlined); ③ multi-step tasks no longer break from ref renumbering or multi-monitor.

### Sprint 2 complete (18/18) — A series + G1–G4 + B6 + C3 + C4 + D1 + D6 + D7 + F3

- [x] **A1** `cu find` predicate query — **done 2026-04-27** (see A1 detail block below)
- [x] **C1** `cu snapshot --diff` cross-snapshot diff — **done 2026-04-27** (see C1 detail block below)
- [x] **A3** `cu snapshot --annotated` labeled screenshot — **done 2026-04-27** (see A3 detail block below)
- [x] **A8** `cu nearest <x> <y>` pixel → ref reverse lookup — **done 2026-04-27** (see A8 detail block below)
- [x] **A9** `cu screenshot --region` region capture — **done 2026-04-27** (see A9 detail block below)
- [x] **A10** `cu snapshot --with-screenshot` tree+image fusion — **done 2026-04-27** (see A10 detail block below)
- [x] **A11** `cu observe-region <x> <y> <w> <h>` candidate refs in a region — **done 2026-04-27** (see A11 detail block below)
- [x] **G1** categorized `cu help` — **done 2026-04-27** (see G1 detail block below)
- [x] **G2** `cu examples [topic]` built-in recipe library — **done 2026-04-27** (see G2 detail block below)

### A series (VLM bridge) — give vision-equipped agents the smoothest hands

- [x] **A3** screenshot overlay with ref numbers (1d) — **done 2026-04-27**
  - **Approach**: `cu snapshot --annotated [--output path]` reuses `screenshot::find_window` for the window_id, calls `CGWindowListCreateImage`, then redraws via **CoreGraphics + CoreText FFI** in a CGBitmapContext: paint the screenshot in, then for each element draw a red border + red-on-white ref label (Helvetica-Bold 14pt × scale). Retina handled automatically (scale = `image_w / window.width`). CG's bottom-up y is flipped per rect manually. Zero new dependencies (CT is a system framework).
  - **Files changed**: `src/screenshot.rs` (+200 lines: CG/CT FFI, `Annotation` struct, `annotate_window`, `build_text_line`), `src/main.rs` (`Cmd::Snapshot` adds `--annotated` / `--output`, `cmd_snapshot` branches), `tests/commands/test_annotated.sh` (new, 17 assertions)
  - **JSON output**: snapshot result gains `annotated_screenshot: <path>` and `image_scale: <ratio>`; image_scale tells the VLM the pixel-to-screen-coord factor (Retina = 2.0).
  - **References landed**: [ghost-os `Perception/Annotate.swift`](https://github.com/ghostwright/ghost-os) — same idea (red box + numeric label), but we use pure CG + CT FFI without an image library.
  - **Value**: single highest-leverage VLM-friendly feature. Codex CU / Anthropic CUA don't have an "AX-ref-as-visual-overlay" path — cu has a real moat here.
  - **Verified**:
    1. ✅ Live Finder snapshot of 30 elements → 1800×1200 PNG, red boxes + numbers clearly readable
    2. ✅ Retina scale=2.0 detected automatically (image_w 1800 / window.width 900 = 2.0)
    3. ✅ Default output path `/tmp/cu-annotated-<ts>.png`, `--output` overrides
    4. ✅ Coexists with plain snapshot (plain doesn't write image, doesn't add fields)
    5. ✅ Orthogonal to `--diff` / `--limit` / other flags
    6. ✅ Human mode also prints `Annotated screenshot: <path>`
  - **Tests**: 310/312 (+17 new; 2 skipped are A6 modal, unrelated to C1 / A3)
  - **Follow-ups**: ① labels can collide in dense regions — collision-aware offset later; ② all elements are boxed today — selective `--annotate-only role=button,row` later; ③ color by role (button red / textfield blue / row green) for faster VLM localization

- [x] **A8** `cu nearest <x> <y>` pixel → ref reverse lookup (0.2d) — **done 2026-04-27**
  - **Approach**: reuses `ax::snapshot()` for the element list, computes "Euclidean distance from point to nearest point on rect" per element (0 if inside). Returns `match` with ref/role/title/value/coords/distance/inside; `--max-distance N` filters; empty match is `match:null` (not an error).
  - **Files changed**: `src/main.rs` (+`Cmd::Nearest` + dispatch + `cmd_nearest` ~85 lines), `tests/commands/test_nearest.sh` (new, 18 assertions)
  - **API**: `cu nearest 480 320 --app X [--limit 200] [--max-distance 50]` → `{"match":{"ref":12,"role":"button","distance":0.0,"inside":true,...}, "query":{"x":480,"y":320}}`
  - **Value**: closes the other half of the VLM↔cu bridge. A3 = "look at image, pick ref"; A8 = "VLM has fixed the coords — translate to ref." Together they cover both "find by visual label" and "give me the absolute coord and translate" workflows.
  - **Verified**:
    1. ✅ Point inside element → returns it, distance=0, inside=true
    2. ✅ Point outside → returns nearest element, distance>0, inside=false
    3. ✅ `--max-distance 10` + faraway point → match=null
    4. ✅ Returned ref consistent with same-limit snapshot
    5. ✅ NaN / nonexistent app → structured error
  - **Tests**: 328/330 (+18 new; 2 still A6 modal skipped)

- [x] **A9** `cu screenshot --region` region capture (0.2d) — **done 2026-04-27**
  - **Approach**: `screenshot.rs` gains `capture_region(x, y, w, h, path)` using `CGWindowListCreateImage` with explicit screenBounds (shares listOption with `--full`). `main.rs` adds `--region` flag and a `parse_region` helper accepting `"x,y WxH"` / `"x,y,w,h"` / `"x y w h"`. Coords are in points (same space as snapshot element coords). Errors: non-numeric / wrong arity / zero or negative size all flow through CuError.
  - **Files changed**: `src/screenshot.rs` (+30 lines for capture_region), `src/main.rs` (`Cmd::Screenshot` adds region, `cmd_screenshot` routes region-first, `parse_region` helper 21 lines), `tests/commands/test_screenshot.sh` (+13 assertions covering region success + Retina scaling + size comparison + 4 error paths)
  - **JSON output**: `{ok, path, mode:"region", offset_x, offset_y, width, height}` — offset_x/y let the agent map image pixels back to screen coords.
  - **Value**: empirical 300×200 point region = 600×400 px PNG = **85 KB (vs full window 471 KB, 5.5× smaller)**. VLMs verifying "did the button turn grey", "is the modal gone" no longer need full-window captures.
  - **Verified**:
    1. ✅ All 4 input formats (with space, with comma, with x, mixed) parse
    2. ✅ Retina ×2 automatic (PNG pixels = region points × scale)
    3. ✅ Region file strictly smaller than full-window file (85 KB < 471 KB)
    4. ✅ Non-numeric / wrong arity / 0×0 / negative → structured CuError
    5. ✅ Orthogonal to `--app` / `--full` / default path (region has top precedence)
  - **Tests**: 344/346 (+13 new; 2 still A6 modal skipped)

- [x] **A10** `cu snapshot --with-screenshot` fused output (0.3d) — **done 2026-04-27**
  - **Approach**: `screenshot.rs` adds `capture_window_with_scale(window, path)` reusing the raw capture image, reads `CGImageGetWidth` for scale, then saves. `main.rs::cmd_snapshot` adds `--with-screenshot` — when set and `--annotated` is not, captures via `capture_window_with_scale` and attaches `screenshot` + `image_scale` to JSON. Both flags together: annotated wins (already includes the image, no plain). The `--diff` paths (first-call and warm) also wire it in — VLM "look at diff for changes + look at image to verify" works as one call.
  - **Files changed**: `src/screenshot.rs` (+25 lines for capture_window_with_scale), `src/main.rs` (`Cmd::Snapshot` + with_screenshot field, cmd_snapshot adds plain_screenshot branch, 3 emission paths attach the field), `tests/commands/test_snapshot_with_screenshot.sh` (new, 24 assertions)
  - **JSON contract**: plain uses `screenshot` + `image_scale`; annotated uses `annotated_screenshot` + `image_scale`; both flags together → annotated field present, screenshot field absent
  - **Value**: tree and image are captured at the **same UI instant** — no race between two `cu` calls causing ref mismatches. Combined with `--diff`, VLMs get "what changed + the current image" in one call.
  - **Verified**:
    1. ✅ plain `--with-screenshot` returns `screenshot` + `image_scale`, no `annotated_screenshot`
    2. ✅ Default output `/tmp/cu-snapshot-<ts>.png`, `--output` overrides
    3. ✅ Plain snapshot without flag has no image fields
    4. ✅ `--annotated` + `--with-screenshot` → annotated wins
    5. ✅ `--diff` + `--with-screenshot` works on first-call and warm paths
    6. ✅ Human mode prints `Screenshot: <path>`
  - **Tests**: 368/370 (+24 new; 2 still A6 modal skipped)

- [x] **A11** `cu observe-region <x> <y> <w> <h>` region element query (0.3d) — **done 2026-04-27**
  - **Approach**: reuses `ax::snapshot()` for the element list; the filter lives in `main.rs::cmd_observe_region`. Three membership modes (`--mode`):
    - `intersect` (default): bbox overlaps the region at all
    - `center`: element center falls inside the region (filters big-container noise)
    - `inside`: bbox fully contained (strictest)
  - **Files changed**: `src/main.rs` (+`Cmd::ObserveRegion` + dispatch + `cmd_observe_region` ~85 lines), `tests/commands/test_observe_region.sh` (new, 22 assertions including mode invariant checks)
  - **JSON output**: `{ok, app, region:{x,y,w,h}, mode, matches:[...], count, scanned, truncated}` — same shape as `find` for jq pipelines
  - **Value**: completes the "VLM visual perception → cu structured candidates" granularity. A8 = single-point lookup (nearest one ref); A11 = region lookup (all candidate refs). Complementary by scenario.
  - **Verified**:
    1. ✅ Live Finder 350×200 region: intersect=92 / center=88 / inside=69 (nesting holds)
    2. ✅ Invariant: every center-mode match has its center in the region; every inside-mode match has its full bbox in the region
    3. ✅ Off-screen region → `count:0 ok:true` (not an error)
    4. ✅ ref consistent with same-limit snapshot
    5. ✅ 0×0 / unknown mode → structured CuError
    6. ✅ Human mode: list + `No elements in region (...)` when empty
  - **Tests**: 390/392 (+22 new; 2 still A6 modal skipped)

### G series (CLI craft) — best practices when an agent uses the CLI directly

- [x] **G1** top-level `cu help` categorization (0.3d) — **done 2026-04-27**
  - **Approach**: clap's `before_help` injects "COMMANDS BY CATEGORY" at the top of every help path (`cu` no-arg / `cu -h` / `cu --help`); 4 categories (Discover / Observe / Act / Script & System) hold 22 commands one per line. Keep `long_about` for workflow narrative + clap's auto-generated flat list → three-layer structure (category quick-read → workflow narrative → detailed command table). Also adds "WORKFLOW FOR VLM AGENTS" to long_about (A3+A8+A11 standard usage).
  - **Files changed**: `src/main.rs` (Cli `before_help` + `long_about` adds VLM workflow), `tests/commands/test_help.sh` (new, 29 assertions: 3 help paths × category visible + flat-list completeness + workflow narrative + subcmd help still works); CLAUDE.md / README.md command count 20 → 22 (set-value/perform were undercounted before)
  - **Why categorize instead of cull**: cu's peer set is `gh` / `kubectl` / `aws` (multi-command Unix CLIs), not the Anthropic CUA / Codex CU "model-direct tool call" paradigm. 22 commands is well within peer norm; the agent-friendly key is **discovery + selection** clarity.
  - **Verified**:
    1. ✅ All three help paths show the categorized block first
    2. ✅ Each new VLM command (find/nearest/observe-region) is in its category
    3. ✅ All 22 commands still appear in clap's flat list
    4. ✅ `--help` still includes the full long_about workflow narrative
    5. ✅ `cu <subcmd> --help` is unaffected
  - **Tests**: 418/421 (+29 new; 2 still A6 modal skipped; 1 wait flake re-ran clean in isolation)

- [x] **G2** `cu examples [topic]` built-in recipe library (0.5d) — **done 2026-04-27**
  - **Approach**: new `cu examples [topic]` with 12 built-in recipes as a `RECIPES: &[(name, summary, body)]` const. No topic: human mode prints aligned "name + summary" table, JSON returns topics array; with topic: prints a 3–10 line working shell snippet (covering launch-app / fill-form / dismiss-modal / read-app-data / wait-for-ui / vlm-click-by-image / vlm-coord-to-ref / vlm-region-candidates / diff-after-action / menu-click / region-screenshot / system-pref). Unknown topic → CuError + lists every legal topic in the hint + suggested_next points back to `cu examples`.
  - **Files changed**: `src/main.rs` (+`Cmd::Examples` + dispatch + RECIPES const + cmd_examples 60 lines), `tests/commands/test_examples.sh` (new, 39 assertions: list shape / 12 topics non-empty / content grep / structured error / human render); CLAUDE.md / README / SKILL.md command count 22 → 23 + categorized help adds examples to Discover
  - **VLM workflow coverage**: `vlm-click-by-image` (A3 annotated → pick ref), `vlm-coord-to-ref` (A8 pixel → ref), `vlm-region-candidates` (A11 region → candidate refs), `region-screenshot` (A9 token-cheap region capture), `diff-after-action` (C1 cheap re-snapshot) — every A-series VLM bridge has a corresponding recipe
  - **Value**: when an agent gets stuck, one command `cu examples dismiss-modal` returns a copyable working example — no need to re-read SKILL.md. Each recipe < 10 lines; the entire library is embedded in the binary, zero extra files.
  - **Verified**:
    1. ✅ All 12 topics return `ok:true` + non-empty body
    2. ✅ Key recipe contents correct (launch-app contains `cmd+space`, vlm-click contains `--annotated`, etc.)
    3. ✅ Unknown topic returns CuError, hint lists all topics, suggested_next points back to `cu examples`
    4. ✅ Human mode aligned-table output + per-topic `# topic — summary` header
    5. ✅ JSON list / JSON detail / human list / human detail — all four renderings correct
  - **Tests**: +39 new; command count starts at 23 (G1 corrected to 22 by counting set-value/perform; G2 adds examples → 23)

- [x] **G3** `cu find --first --raw` directly outputs the ref integer (5 minutes) ✅ 2026-04-27
  - **Approach**: `--raw` makes `cu find` print bare ref integers on stdout (one per line), no jq needed; no-match exits 1 with no output.
  - **Value**: `cu click $(cu find --app X --role button --title-equals Save --first --raw)` works in one line.
  - **Tests**: `tests/commands/test_find.sh` +4 assertions (`--first --raw` single integer, multi-line integers, no-match exits 1, pipe-friendly)

- [x] **G4** add a "PREFER:" block to each subcommand's `after_help` (0.3d) ✅ 2026-04-27
  - **Approach**: 7 subcommands with overlapping use cases gain a `PREFER:` block in `after_help` (agent runs `cu <cmd> --help` and sees the selection guide directly) —
    - `cu set-value` → prefer over `cu type` for AX textfields/textareas/comboboxes
    - `cu type` → prefer `cu set-value` for AX textfields; use type for non-AX (Electron) or focus-already-set keystroke flows
    - `cu perform` → use `cu click` for the common case, only use perform for non-AXPress actions
    - `cu tell` → prefer over click/type for scriptable apps (`S` flag in `cu apps`)
    - `cu find` → prefer over `cu snapshot | grep`
    - `cu nearest` → VLM visual coords → ref reverse lookup
    - `cu observe-region` → VLM-narrowed region → enumerate candidate refs
  - **Tests**: `tests/commands/test_help.sh` +7 assertions (each command's --help contains a `^PREFER:` block)

### Original Sprint 2 tasks (closed-loop precision) — deferred behind A/G

- [x] **B6** AX-based window raise replaces global activate (0.5d) ✅ 2026-04-27
  - **Approach**: `src/ax.rs::raise_window(pid)` — get `AXMainWindow` / `AXFocusedWindow`, set `AXMain=true` + `AXRaise`, zero AppleScript. `cu window focus` defaults to this; AX failure falls back to System Events behavior.
  - **Response**: returns `method: "ax-raise"` (success) / `"applescript-frontmost"` (fallback)
  - **Tests**: `tests/commands/test_window.sh` +1 assertion (`focus uses method=ax-raise`)

- [x] **C3** `cu wait` advanced conditions (1d) ✅ 2026-04-27
  - **Approach**: `wait::Condition` gains `NewWindow` / `Modal` / `FocusedChanged` variants; the main loop captures a baseline on first poll. `NewWindow` calls `ax::window_count(pid)` (queries the `AXWindows` array) directly — doesn't depend on snapshot.elements (focused window only); `Modal` reads `snap.modal`; `FocusedChanged` compares `snap.focused.ref` to baseline.
  - **CLI flags**: `--new-window` / `--modal` / `--focused-changed` (mutually exclusive with the existing `--text` / `--ref` / `--gone`)
  - **Tests**: `tests/commands/test_wait_advanced.sh` 8 assertions (error paths, timeout timing, dynamic new-window detected ~1.2s)

- [x] **D1** multi-monitor coordinate first-class handling (1d) ✅ 2026-04-27
  - **Approach**: new `src/display.rs` — `CGGetActiveDisplayList` + `CGDisplayBounds` + `CGMainDisplayID`, returns `Vec<DisplayInfo{id, main, x, y, width, height}>`. `cu snapshot` injects `displays` at top level on every JSON output path (plain / `--diff` first / `--diff` warm); agents can resolve element (x,y) to a display themselves.
  - **API**: `display::list()` / `display::display_for_point(x, y, &displays)` (the latter is reserved for future mouse verification)
  - **Tests**: `tests/commands/test_displays.sh` 7 assertions (array shape, exactly-one-main, diff path, main-screen bounds plausibility)

- [x] **D6** app launch + wait primitive (0.5d) ✅ 2026-04-27
  - **Approach**: `cu launch <name|bundleId>` invokes `open -a` / `open -b` to launch, polls every 100ms for AX-reported main/focused window. `--no-wait` skips the wait, `--timeout` exits 1 on timeout. Bundle id resolves through `system::resolve_by_bundle_id` (System Events `whose bundle identifier is`) to `(pid, name)`.
  - **Response**: `{ok, app, pid, ready_in_ms, waited, window:{x,y,width,height}}`
  - **Tests**: `tests/commands/test_launch.sh` 16 assertions (name path, bundle-id path, warm/cold, no-wait, error, human mode)

- [x] **D7** single-shot AXObserver wait replaces fixed 500ms (0.5d, new) ✅ 2026-04-27
  - **Approach**: new `src/observer.rs` (~180 lines FFI), `maybe_attach_snapshot` entry switches to `observer::wait_for_settle(pid, POST_ACTION_DELAY_MS)`: `AXObserverCreate` → subscribe to `AXValueChanged` / `AXFocusedUIElementChanged` / `AXMainWindowChanged` / `AXSelectedChildrenChanged` → `CFRunLoopRunInMode` 50ms slices, return on first notification; on timeout, fall back to sleep. Observer lives only for the call (no daemon).
  - **Response**: every action response gains `settle_ms` recording the actual wait
  - **Value**: typical `settle_ms` ≈ 50–200ms (vs the old fixed 500ms), still capped at 500ms to avoid runaway
  - **Tests**: `tests/commands/test_settle.sh` 6 assertions (present, integer, ≤cap, max-of-3 sample, omitted with --no-snapshot)

- [x] **C4** action `method` gains `confidence` + `advice` fields (0.2d) ✅ 2026-04-27
  - **Approach**: `src/main.rs::method_meta(method)` → `(confidence, advice)` table, applied by `annotate_method` in `maybe_attach_snapshot` to every action response. `ax-action`/`ax-set-value`/`ax-perform`/`*-pid` = high; `ocr-text-pid` = medium + verify advice; `*-global` = low + "pass --app" advice.
  - **Tests**: `tests/commands/test_method_meta.sh` +8 assertions (key with --app=high/no-advice, no-app=low/has-advice, set-value=ax-set-value/high)

- [x] **F3** SKILL.md upgraded to cookbook + decision tree (0.5d) ✅ 2026-04-27
  - **Approach**: ① top adds Decision Tree (goal-shaped tree → command) + Hard Rules; ② bottom adds 10-recipe Cookbook (launch / scriptable read / set-value / find-by-label / VLM-coord-click / observe-region / region-screenshot / wait-conditions / snapshot-diff / defaults); ③ command count bumped to 24, Output Format adds method/confidence/advice/settle_ms/displays field notes
  - **Value**: agent can locate "which command do I need now" at a glance, less prompt-time trial-and-error

- [x] **A1** `cu find --role X --title-contains Y` command (0.5d) — **done 2026-04-27**
  - **Approach**: reuses `ax::snapshot()`'s walker; the filter lives in `main.rs::cmd_find`. Zero new walker code, returned ref matches same-limit snapshot exactly → `cu click <ref>` works directly. All filters AND-combined: `--role` (normalized lowercase, e.g. `button` / `row`), `--title-contains` (case-insensitive substring), `--title-equals` (exact), `--value-contains` (case-insensitive substring). `--first` returns single `.match` object (good for `... | jq -r .match.ref | xargs cu click`). Empty result is `ok:true count:0` — not an error. 0 filters returns a structured error (CuError + suggested_next).
  - **Files changed**: `src/main.rs` (+`Cmd::Find` + dispatch + `cmd_find` ~100 lines), `tests/commands/test_find.sh` (new, 24 assertions), SKILL.md / README.md / CLAUDE.md (command count 17 → 18 + new "Targeted query" section)
  - **References landed**: [kagete `find`](https://github.com/ringotypowriter/kagete)
  - **Verified**:
    1. ✅ `cu find --app Finder --role row` returns all rows, scanned/truncated fields populated
    2. ✅ `cu find --first` returns single `.match` object, null when empty
    3. ✅ AND filtering narrows correctly (role=row + title-contains strictly ⊆ role=row alone)
    4. ✅ Returned ref + coords match same-limit snapshot exactly
    5. ✅ Case-insensitive (lowercase / UPPERCASE same count)
    6. ✅ 0 filters / nonexistent app → structured error
  - **Tests**: 320/320 (296 existing + 24 new)

- [x] **C1** Diff snapshot (0.5d) — **done 2026-04-27**
  - **Approach**: new `cu snapshot --diff`, standalone (not injected into action commands — keep change small + composable). New `src/diff.rs` — cache path `/tmp/cu-snapshot-cache/<pid>.json`; element identity = `(role, round(x), round(y))`, robust to ref renumbering, sensitive to window movement. `content_changed` = title / value / size changed (width/height tolerance 0.5px). First call has no cache → returns full snapshot + `first_snapshot:true`, agent knows diffs start working from the next call. `Element` gets `Deserialize + Clone` for round-trip.
  - **Files changed**: `src/diff.rs` (new, 92 lines), `src/ax.rs` (Element derives), `src/main.rs` (`Cmd::Snapshot` adds `--diff` flag, `cmd_snapshot` branches, `print_diff_human` uses `+ ~ -`), `tests/commands/test_snapshot_diff.sh` (new, 21 assertions), SKILL.md / README.md (usage section)
  - **References landed**: own design — no open-source peer does this; another industry-first for cu
  - **Verified**:
    1. ✅ First call: `first_snapshot:true` + full elements
    2. ✅ Second call no-change: `+0 ~0 -0`, unchanged_count = total elements
    3. ✅ After set-value: precisely captures the textarea as `~`, other 19 untouched
    4. ✅ Cache file written to `/tmp/cu-snapshot-cache/<pid>.json`
    5. ✅ Human mode uses `+ [ref] role`, `~ [ref] role`, `- [ref] (removed)` + Summary line
    6. ✅ `--diff` coexists with plain snapshot (plain doesn't break cache consistency)
  - **Tests**: 293/295 (2 skipped are A6 modal-trigger suppressed by macOS iCloud auto-save — switched to _skip rather than fail; unrelated to C1)
  - **Follow-up**: `--diff-snapshot` flag on action commands (replacing the default full snapshot injection) — wait until real agents have used `cu snapshot --diff` in the wild, decide based on feedback
  - **Known limitation**: window movement makes every element's identity change → all added+removed. Inherent trade-off of identity-by-position; agents in multi-step flows should avoid moving the window (or treat the first diff after move as `first_snapshot`).

---

## Sprint 3 — long-term capabilities + observability (5+ days)

- [x] **A2** axPath stable selector (2d) ✅ 2026-04-27
  - **Approach**: `src/ax.rs` walker computes an `axPath` field per Element on DFS, formatted `Role[Title]:N/Role[Title]/...` (`[Title]` optional, `:N` is the 0-indexed sibling occurrence). CLI adds `--ax-path` to `cu click` / `cu set-value` / `cu perform`, each routing to `ax::resolve_by_ax_path` / `ax::ax_set_value_by_path` / `ax::ax_perform_by_path` — top-down resolvers re-walking the AX tree segment by segment, no dependence on ref numbering.
  - **Format conventions**: `/` `[` `]` in titles are replaced with `_`; titles longer than 60 chars are truncated + `…`; default `:0` is omitted.
  - **Value**: multi-step flows are no longer broken by ref renumbering. Capture all needed axPaths in a single snapshot, then drive subsequent steps with `--ax-path` — stable across snapshots.
  - **Tests**: `tests/commands/test_ax_path.sh` 11 assertions (every element has axPath, `:N` appears and is unique, coord round-trip matches snapshot, error paths, set-value rejects read-only elements, perform errors when selector missing)

- ~~**A3** screenshot overlay with ref numbers~~ — **moved to Sprint 2 first slot**

- [x] **D8** AX bridge warm-up (0.3d) ✅ 2026-04-27
  - **Approach**: `cmd_launch` calls `ax::snapshot(pid, &name, 5)` after the window appears; response includes `warmup_ms`. New `cu warm <app>` lets the user manually warm a manually-opened app.
  - **Background**: TextEdit / Mail and others have a 200–500ms cold-start AX-walk delay that hits the first click/snapshot.
  - **Tests**: `tests/commands/test_warm.sh` 8 assertions + new `warmup_ms` assertion in `test_launch.sh`

- [x] **B7** failure diagnostic `cu why` (0.5d) ✅ 2026-04-27
  - **Approach**: new `ax::inspect_ref(pid, ref_id)` walks the AX tree to extract AXEnabled / AXFocused / AXSubrole / supported actions; `cu why <ref> --app <name>` assembles a structured `{ found, element, checks, advice }` — checks include in_snapshot / in_window_bounds / click_supported / modal_present, and advice covers modal blocking / disabled / no AXPress / sandbox limitations.
  - **Value**: after click returns `ok:false` (or returns ok but the UI didn't change), one `cu why` call tells the agent "ref doesn't exist / element disabled / doesn't support AXPress, try perform / sandbox app needs a different path" — far less exploratory grepping.
  - **Tests**: `tests/commands/test_why.sh` 15 assertions (found, missing ref, non-running app, human mode)

- ~~**A5** Chrome CDP bridge (3d)~~ — **dropped** (2026-04-27), see Out of Scope

- [ ] **E1** macOSWorld baseline run + publish (1d)
  - **Approach**: run baselines for GPT-5.4 / Claude Opus 4.7 / Sonnet 4.6, publish to README
  - **Existing groundwork**: `tests/macosworld/` + `tests/agent/caliper_records.json` (untracked) + `tests/agent/caliper_report.py` (untracked)
  - **Acceptance**: README has the baseline table + link to a reproducible script

- [ ] **E2** regression dashboard (1d)
  - **Approach**: each release auto-runs a macOSWorld subset, archives results, diffs against last run

---

## What's already shipped (vs Codex CU craft comparison)

> Not just a checklist — each item includes the detailed delta with Codex CU as a basis for retro and continuous improvement.

### ✅ AX-tree first + screenshot-as-fallback (A0)
- **cu**: `src/ax.rs` 919 lines + `src/screenshot.rs` 299 lines
- **vs Codex CU**:
  - `cu snapshot` is flat text (one ref per line); Codex CU outputs an indented tree → cu is more token-efficient and faster for the model to pick a ref, but loses some hierarchy info
  - cu enforces `--limit` (default 50); Codex CU appears to auto-trim → cu is more controllable but sometimes needs multiple snapshots
  - cu attaches a PNG field by default; Codex CU doesn't → cu is more "one-stop" but more tokens

### ✅ AX action chain (B0)
- **cu**: `src/ax.rs:553` `try_ax_actions`, **15 steps** (AXPress → AXConfirm → AXOpen → AXPick → AXShowAlternateUI → child action → checkbox toggle → AXSelected → parent-row select → focus+press → ancestor press → CGEvent)
- **vs Codex CU / open-source peers**:
  - finer than [iFurySt/open-codex-computer-use](https://github.com/iFurySt/open-codex-computer-use) (only checks `prettyActions` + AXPress on click)
  - not on the same abstraction layer as ghost-os (which uses AXorcist's high-level `PerformActionCommand`)
  - **cu leads the field on this axis**

### ✅ Silent window screenshot (A0 sub-item)
- **cu**: `src/screenshot.rs` uses ScreenCaptureKit (primary, via `sck.rs`) + `CGWindowListCreateImage` (fallback) — no activation
- **vs Codex CU**: equivalent (Codex CU also uses ScreenCaptureKit)

### ✅ Auto-snapshot after action (C0)
- **cu**: `maybe_attach_snapshot` is called from every action command, with single-shot AXObserver wait (D7) capping at 500ms
- **vs Codex CU**:
  - Settle strategy: cu uses single-shot AXObserver via D7 (50–200ms typical, 500ms cap); Codex CU is presumed to use long-lived `AXObserverCreate` (faster on average)
  - Reverse opt-out: cu offers `--no-snapshot`, Codex CU doesn't expose one

### ✅ Retina / scale handling (D2)
- **cu**: `screenshot.rs` outputs `offset_x` / `offset_y` / `scale`
- **vs Codex CU**: equivalent

### ✅ OCR fallback (A0 sub-item)
- **cu**: `src/ocr.rs` calls macOS Vision via objc2
- **vs Codex CU**: equivalent; among open-source peers only [axon](https://github.com/gxcsoccer/axon) also uses Vision

### ✅ Three-tier hybrid architecture (AppleScript → AX → screenshot)
- **cu**: `cu tell` / `cu sdef` / `cu snapshot` / `cu click` / `cu screenshot` are exposed by tier
- **vs Codex CU**: Codex CU doesn't emphasize the AppleScript tier (likely uses it internally); cu's promotion of AppleScript to a first-class channel for scriptable apps is a unique choice
- **vs open-source peers**: [axon](https://github.com/gxcsoccer/axon) has the same three-tier architecture (Swift implementation)

### ✅ Single binary, zero runtime dependencies
- **cu**: pure Rust + system-framework FFI
- **vs Codex CU**: Codex CU is a macOS app + cloud; cu is a CLI, lighter to distribute
- **vs open-source peers**: every Swift project needs `swift build`; Node projects need cliclick + pyobjc

### ✅ Full test infrastructure
- **cu**: 700+ command-test assertions (`tests/commands/run_all.sh`) + agent E2E (`tests/agent/run.py`, runs every release) + macOSWorld (`tests/macosworld/`, 133 local tasks)
- **vs open-source peers**: kagete / axon / ghost-os have weaker coverage; cu has the most thorough setup

---

## Explicit Out of Scope

| Item | Codex CU has it? | Why we don't do it |
|---|---|---|
| Soft-cursor overlay (virtual cursor — user sees the agent operating but isn't interrupted) | ✅ signature UX | needs a SwiftUI/AppKit helper process, violates the "zero runtime dependencies" principle |
| MCP server mode | ❌ (not MCP) | violates CLAUDE.md's explicit "CLI only, no MCP" rule |
| ghost-os style record/replay self-learning recipes | ❌ | beyond current product scope, needs a separate product decision |
| Built-in VLM fallback (cu calling a VLM directly) | 🟡 GPT-5.4 ships with one | our agents have their own vision (Claude / GPT); cu is the reusable "hand", shouldn't redundantly call a remote VLM |
| Long-lived daemon + AXObserver push | ✅ internal architecture | violates the "single-binary CLI" philosophy; D7 (single-shot AXObserver wait) takes 80% of the value, the remaining daemon win isn't worth the complexity |
| Co-trained model + tool integration | ✅ Codex CU's core moat | we're a model-agnostic tool, deliberately not bound to a specific model — this is cu's only moat against Codex CU |
| Chrome CDP bridge (DevTools Protocol path for Chrome/Edge/Electron) | 🟡 ghost-os has it | users would have to manually set `--remote-debugging-port=9222` — the UX is already degraded; cu's unified abstraction is "any macOS app", and a Chrome side door dilutes that consistency. AX tree + cu tell already cover 95% of browser ops; the remaining 5% isn't worth 3 days |

> Key insight: cu and Codex CU are **strategically different**. Codex CU goes the "model + tool integrated, closed-product" route; cu goes the "any agent + any shell, zero integration cost, open-tool" route. The optima for each are often the opposite (daemon vs CLI, built-in VLM vs leaving vision to the agent, training co-evolution vs model-agnostic). Sprint 2's design principle is **without abandoning cu's strategy, make the experience for VLM agents using cu the best it can be**.

---

## Major reference projects

| Project | Stars | Language | Borrowed module | Main value |
|---|---|---|---|---|
| [iFurySt/open-codex-computer-use](https://github.com/iFurySt/open-codex-computer-use) | 555 | Swift | `InputSimulation.swift` / `ComputerUseService.swift` / `AccessibilitySnapshot.swift` | most direct open-source replica of Codex CU; primary template for B1/B2/B5/B6/A4/D1 |
| [ringotypowriter/kagete](https://github.com/ringotypowriter/kagete) | 2 | Swift | `Input.swift` / `AXRaise.swift` / `find` / `set-value` / `action` commands | CLI command-design model, axPath inspiration |
| [ghostwright/ghost-os](https://github.com/ghostwright/ghost-os) | 1412 | Swift | `Annotate.swift` / `CDPBridge.swift` / `ToolResult` | screenshot annotation, Chrome enhancement, rich error returns |
| [gxcsoccer/axon](https://github.com/gxcsoccer/axon) | 0 | Swift | `perform` command design | B4 command shape |
| [bradthebeeble/mcp-macos-cua](https://github.com/bradthebeeble/mcp-macos-cua) | 0 | Node.js | productized onboarding | inspiration for a `/cua` skill auto-permission bootstrap |

---

## Progress overview

| Sprint | Status | Start / End | Notes |
|---|---|---|---|
| Sprint 1 — non-disruptive UX + tool surface | **complete** | 2026-04-27 | 10/10 tasks (incl. F2 closed; F2a + F2b + E3 equivalent done) |
| Sprint 2 — VLM bridge + CLI craft + closed-loop precision | ✅ complete | 2026-04-27 | 18/18: A series (5) + A1/C1 + G1–G4 + B6/C3/C4/D1/D6/D7/F3; **27 commands, 700+ test assertions** |
| Sprint 3 — long-term capabilities | in progress | 2026-04-27 — | A2 axPath + D8 AX warmup + B7 cu why done (v0.4.0 release); A5 dropped (CDP); R1–R7 reliability batch + A SCK + capture-protected (v0.5.2); E1 / E2 pending |
