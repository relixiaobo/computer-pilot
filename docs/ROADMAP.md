# Computer Pilot — Codex CU 对齐路线图

> **目标**：让 `cu` 在 macOS 原生 app 场景上达到 Codex Computer Use 同等的"低打扰 + 高成功率"体验。
> **范围**：模型层不在我们战场；这份路线图只覆盖工具层（perception / action / loop / robustness / infra / DX）。
> **追踪方式**：每完成一项打勾，PR 链接附在条目末尾，"完成日期"填实际合并日。
> **配套文档**：
> - [`competitive-analysis.md`](./competitive-analysis.md) — 多项目特性栅格（事实快照）
> - 本文档 — 执行计划 + 进度追踪（动态）

最后更新：2026-04-27（**Sprint 1 + Sprint 2 完结**；Sprint 3 进行中 — A2 axPath / D8 AX warmup / B7 cu why 完成；26 命令、544 测试）

---

## 当前差距快照（vs Codex CU）

| 维度 | cu 现状 | Codex CU | 差距 |
|---|---|---|---|
| 模型 | 解耦，外部 agent 选 | GPT-5.4 内置 | 不在我们战场 |
| AX 树为主 | ✅ | ✅ | 无 |
| AX 动作链 | ✅ 15 步（`src/ax.rs:553`），**比开源同类更细** | ~推测同档 | 无（甚至略胜） |
| 动作不抢焦 | ✅ PID-targeted（B1+B5） | ✅ PID-targeted | 已对齐 |
| 键盘不污染剪贴板 | ✅ Unicode CGEvent（B2） | ✅ unicode + postToPid | 已对齐 |
| `set-value` / `perform` 一等命令 | ✅ B3 + B4 | ✅ | 已对齐（`find` 留给 Sprint 2 A1） |
| 闭环错误 hint | ✅ CuError 结构化（C2） | ✅ 结构化 | 已对齐 |
| 焦点 / Modal 摘要 | ✅ A4 + A6 | 部分 | 已对齐（A6 cu 独有） |
| 路径审计字段 | ✅ method 字段细分（F2a） | ✗ | **cu 独有** |
| 多显示器 | 🟡 单屏 OK | ✅ | 中（Sprint 2 D1） |
| 软光标 overlay | ❌ | ✅ | 不做（违反零依赖） |
| 测试基建 | ✅ 296 命令测试 + agent E2E | 闭源 | cu 略胜 |

**核心判断**：cu 路线选对了，差距集中在"最后一公里工艺" —— Sprint 1（3 天）能补齐体感最强的部分。**Sprint 1 已完成（2026-04-27）**：所有"动作不抢焦 / 键盘不污染剪贴板 / 错误结构化 / 焦点 + Modal 摘要 / set-value + perform 一等命令"差距全部补齐，并且在 method 字段审计 / Modal 警告两处反超 Codex CU。

---

## Sprint 1 — 不抢焦体验 + 工具暴露面（目标 3 天）

完成验收：所有动作命令默认不动用户真光标、不抢全局 frontmost、不污染剪贴板；`set-value` / `perform` / `find` 三个一等命令上线。

- [x] **B1** `mouse.rs` 加 PID-targeted CGEvent 路径（0.5d）— **完成 2026-04-27**
  - **做法**：扩展所有 `mouse::*` 函数签名加 `target_pid: Option<i32>`；`Some(pid)` 走 `CGEventPostToPid` + 新建的 combined-session `CGEventSource`（RAII `EventSource` 包装），`None` 走原全局 `cghidEventTap`。`cmd_click` 三种模式（OCR / coords / ref）在已知目标 pid 时全部传 `Some(pid)`
  - **变更文件**：`src/mouse.rs`（FFI + EventSource RAII + 5 个公共函数签名），`src/main.rs`（cmd_click 三模式接线，cmd_scroll/hover/drag 暂传 None 留 B5）
  - **参考已落实**：
    - [iFurySt/open-codex-computer-use `InputSimulation.swift`](https://github.com/iFurySt/open-codex-computer-use/blob/main/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/InputSimulation.swift) — `clickTargeted()` + `.combinedSessionState`
    - [ringotypowriter/kagete `Input.swift:51`](https://github.com/ringotypowriter/kagete/blob/main/Sources/kagete/Input.swift) — `click(toPid:)`
  - **验证**（`tests/commands/verify_no_disruption.sh`）：
    1. ✅ `cu click 5 5 --app Finder` → 光标 + frontmost 完全没变
    2. ✅ `cu click 1 --app Finder`（AX 路径）→ 同上
    3. ✅ 对照组 `cu click 7 7`（无 --app）→ 光标从 (450,81) 拽到 (7,975)，frontmost 仍是 Ghostty（全局路径仍工作）
  - **测试**：258/258 命令测试全绿

- [x] **B2** `cu type` / `cu key` 加 PID-targeted unicode 路径（0.5d）— **完成 2026-04-27**
  - **做法**：`key.rs` 新增 `type_text(text, target_pid)` 用 `CGEventKeyboardSetUnicodeString` + `CGEventPostToPid`，UTF-16 char-by-char、3ms 间隔；`key::send` 扩 `target_pid` 参数。`cmd_type` / `cmd_key` 在 `--app` 时走 PID 路径；删除原 `system::type_text`（剪贴板粘贴）和 `system::send_key`（AppleScript activate）—— git 历史里能找回，作为未来 sandbox app fallback 时的参考
  - **变更文件**：`src/key.rs`（FFI + EventSource RAII + `type_text` + `send` 签名扩展），`src/main.rs`（cmd_type/cmd_key 接线），`src/system.rs`（删 145 行 dead code + unused import 清理）
  - **参考已落实**：
    - [iFurySt `InputSimulation.swift:typeText(_:pid:)` / `pressKey(_:pid:)`](https://github.com/iFurySt/open-codex-computer-use/blob/main/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/InputSimulation.swift) — 完整可抄
    - [kagete `Input.swift`](https://github.com/ringotypowriter/kagete/blob/main/Sources/kagete/Input.swift) — 3ms 间隔的经验值
  - **验证**（`verify_no_disruption.sh` 已升级到带 cursor parking + tolerance 的鲁棒版）：
    1. ✅ `cu type "..." --app Finder` → 光标不漂、剪贴板 sentinel 不被覆盖
    2. ✅ `cu key escape --app Finder` → 光标不漂、frontmost 仍是 Ghostty
    3. ✅ 对照 `cu click 7 7`（无 --app）→ 光标精确warp 到 (7, 975)
  - **测试**：258/258 命令测试全绿

- [x] **B5** PID-targeted scroll / hover / drag（0.3d）— **完成 2026-04-27**
  - **做法**：B1 已经把 `mouse::scroll` / `hover` / `drag` 函数签名扩成了 `target_pid: Option<i32>`；本步只补 CLI 层 —— 给 `Cmd::Scroll` / `Hover` / `Drag` 加 `--app` 参数，handler 解析 pid 后传 `Some(pid)`
  - **变更文件**：`src/main.rs`（3 个 enum 变体加字段 + 3 个 dispatch 解构 + 3 个 cmd 函数加参数）
  - **参考已落实**：[iFurySt `InputSimulation.swift:scrollTargeted()` / `dragTargeted()`](https://github.com/iFurySt/open-codex-computer-use/blob/main/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/InputSimulation.swift)
  - **验证**（`verify_no_disruption.sh` 新增 3 项）：
    1. ✅ `cu scroll down 1 --x 500 --y 500 --app Finder` → 光标不漂
    2. ✅ `cu hover 100 100 --app Finder` → 光标不漂（hover 的 PID 路径只 dispatch mouseMoved 给目标进程，不动真光标）
    3. ✅ `cu drag 100 100 200 200 --app Finder` → 光标不漂

- [x] **B3** `cu set-value <ref> "text"` 一等命令（0.3d）— **完成 2026-04-27**
  - **做法**：`ax.rs` 新增 `pub fn ax_set_value(pid, ref_id, limit, value)` —— 复用 `find_element_by_ref` 的 walker 模式写了一个 `find_and_set_value` 变体，调内部已存在的 `try_set_value(element, "AXValue", cfstr(value))`；新 CLI 命令 `cu set-value <ref> <value> --app X` 直接调用。失败时返回结构化 hint（"try `cu click <ref>` to focus then `cu type` instead"）
  - **变更文件**：`src/ax.rs`（+50 行：`find_and_set_value` + `ax_set_value`），`src/main.rs`（+30 行：`SetValue` enum 变体、dispatch、`cmd_set_value`），`tests/commands/test_set_value.sh`（+102 行新测试套件，15 个断言）
  - **参考已落实**：[kagete `set-value` 命令](https://github.com/ringotypowriter/kagete) —— 行业唯一一等命令样板
  - **验证**：
    1. ✅ `cu set-value 1 "..." --app TextEdit` 直接写入文档（AppleScript readback 验证）
    2. ✅ Unicode（中文 `你好世界`）正确（NFC-normalized 比对）
    3. ✅ 多次写入是覆盖不是追加
    4. ✅ TextEdit 不被切到前台、用户光标不漂、剪贴板不污染（在 `verify_no_disruption.sh` 里）
    5. ✅ ref=0 / ref 不存在 / 元素不可写 → 返回 `{ok:false, error, hint}` 结构化错误
  - **测试**：273/273 命令测试全绿（+15 新增）

- [x] **B4** `cu perform <ref> <AXAction>` 通用命令（0.3d）— **完成 2026-04-27**
  - **做法**：`ax.rs` 新增 `pub fn ax_perform(pid, ref_id, limit, action)`；同时新增 `AXUIElementCopyActionNames` FFI 和 `copy_action_names` 辅助 —— 失败时把元素**实际支持**的 actions 列表打回去给 agent；新 CLI 命令 `cu perform <ref> <action> --app X`
  - **变更文件**：`src/ax.rs`（+90 行：FFI、`copy_action_names`、`find_and_perform_action`、`ax_perform`），`src/main.rs`（`Cmd::Perform` + dispatch + `cmd_perform`），`tests/commands/test_perform.sh`（+85 行，17 个断言）
  - **参考已落实**：
    - [gxcsoccer/axon `perform`](https://github.com/gxcsoccer/axon) — 命令形态
    - [kagete `action`](https://github.com/ringotypowriter/kagete) — 设计理念
  - **验证**：
    1. ✅ `cu perform 1 AXShowDefaultUI --app Finder` 成功，返回 `available_actions`
    2. ✅ `cu perform 1 AXBogus --app Finder` 失败时返回结构化 hint + suggested_next（含元素实际支持的 actions）
    3. ✅ ref 0 / ref 不存在 / 错误 action 全部走 C2 的结构化错误路径
  - **测试**：290/290 命令测试全绿（+17 新增）

- [x] **C2** 失败 hint 结构化（0.5d）— **完成 2026-04-27**
  - **做法**：新建 `src/error.rs::CuError { error, hint, suggested_next, diagnostics }`，提供 fluent builder（`CuError::msg(...).with_hint(...).with_next(...).with_diagnostics(...)`）。`From<String>` / `From<&str>` 让现有代码体不用改，只需把所有 `Result<(), String>` 升级为 `Result<(), CuError>`（20 处机械替换 + 9 处 `Err(string)` 加 `.into()`）。`main()` 的错误 formatter 输出全字段 JSON，human mode 输出 `Error / Hint / Try` 三行。`ax_set_value` 和 `ax_perform` 失败路径已用上 builder
  - **变更文件**：`src/error.rs`（新建 80 行），`src/main.rs`（mod 引用、20 处签名替换、9 处 `.into()`、错误 formatter 重写），`src/ax.rs`（导入 CuError，`ax_set_value` / `ax_perform` 用 builder）
  - **参考已落实**：
    - [ghostwright/ghost-os `Common/Types.swift:ToolResult`](https://github.com/ghostwright/ghost-os) — `success/error/suggestion` 结构
    - Anthropic 工具 API 错误返回哲学（详细到模型直接 retry）
  - **验证**：
    1. ✅ `cu set-value 1 "x" --app Finder` 失败 → `{ok:false, error, hint:"...", suggested_next:[...]}`
    2. ✅ `cu perform 1 AXBogus --app Finder` 失败 → 上述全字段 + `diagnostics.available_actions` 列表
    3. ✅ 测试断言所有结构化字段都正确传递
  - **后续**：其他 cmd_* 失败路径（cmd_click 元素不存在、cmd_type 非法字符等）目前还是裸 String 错误，可在 Sprint 2 时酌情补 hint。框架已就位

- [x] **A4** snapshot 顶部加 Focused 摘要（0.2d）— **完成 2026-04-27**
  - **做法**：snapshot 内部读 `AXFocusedUIElement` of app，提取 role/title/value/position，按 (role, x, y) 在已收集的 elements 列表里反查 ref_id（match within 1px tolerance）。如果焦点元素超出 `--limit` 窗口，仍然返回 role/title/value 但 ref=None。新结构 `FocusedSummary { ref?, role, title?, value? }` 加到 `SnapshotResult.focused`。Human mode 输出 `Focused: [N] role "title" value="..."` 第一/二行
  - **变更文件**：`src/ax.rs`（+30 行：`FocusedSummary` struct + `detect_focused` helper），`src/main.rs`（`print_snapshot_human` 多 9 行渲染）
  - **参考已落实**：[iFurySt `AccessibilitySnapshot.swift:focusedSummary`](https://github.com/iFurySt/open-codex-computer-use/blob/main/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/AccessibilitySnapshot.swift)
  - **验证**：实测 `cu --human snapshot TextEdit` 输出 `Focused: [1] textarea "" value="..."` —— 焦点字段 + ref + 当前内容

- [x] **A6** snapshot 顶部 Modal 警告（0.3d）— **完成 2026-04-27**
  - **做法**：snapshot 中调 `detect_modal(window_el)` —— 先看窗口本身的 AXRole/AXSubrole（AXSheet / AXSystemDialog / AXDialog），再扫窗口直接子节点找 AXSheet。命中则返回 `ModalSummary { role, subrole?, title? }` 加到 `SnapshotResult.modal`。Human mode 输出 `⚠ Modal: AXSheet "..."` 醒目警告
  - **变更文件**：`src/ax.rs`（+45 行：`ModalSummary` struct + `detect_modal` helper），`src/main.rs`（`print_snapshot_human` 多 8 行渲染）
  - **参考**：自实现（开源同类没有这个能力）
  - **验证**：实测在 TextEdit 触发 Cmd+W 关闭未保存文档，snapshot 顶部出现 `⚠ Modal: AXSheet ""`，agent 能立即知道要先处理 sheet 再做其他操作
  - **附加价值**：sheet 内的元素自动成为 snapshot 主体（focused 也自动更新到 sheet 里的输入框），形成完整的"已就绪可操作"视图

- [~] **F2** `--background` 全局 flag — **关闭，不实现（2026-04-27）**
  - **关闭理由**：系统评估后认为多此一举。B1/B2/B5 已经把"是否 PID-targeted"的开关做到了 `--app` 这一层 —— 给 `--app` 即不打扰，不给即默认全局；agent 已经因为可靠性原因被强烈鼓励永远带 `--app`，所以"非打扰"路径其实**已经是默认**。再加一个 `--background` 全局 flag 反而引入语义重复（`--background` + 无 `--app` 时该走哪条？），并且破坏 "method 字段已经是单一事实源" 的设计
  - **替代方案**：把"路径透明度"问题用 F2a 解决（method 字段细分），把"用户/agent 知道为什么"用 F2b 解决（文档化 focus model）
  - **不实现的取舍**：iFurySt 用环境变量是因为他们整个 binary 默认就是 PID-targeted、需要 opt-in 才能 fall back；cu 是 CLI 工具、按调用语义路由更直接，不需要全局开关

- [x] **F2a** 细化 method 字段，区分 PID 与 global 路径（0.1d）— **完成 2026-04-27**
  - **做法**：所有动作命令的 JSON 响应里的 `method` 字段都加上路径后缀。`cmd_click` 三模式：ref 模式总是 `ax-action` / `cgevent-pid` / `cgevent-right-pid` / `double-click-pid`（已知 pid）；coord 模式按 `--app` 在 `cgevent-pid` / `cgevent-global` / `double-click-{pid,global}` / `cgevent-right-{pid,global}` 间选；OCR 模式 `ocr-text-pid` / `ocr-text-global`。`cmd_type` 加 `unicode-pid` / `unicode-global`，`cmd_key` 加 `key-pid` / `key-global`，`cmd_scroll` / `hover` / `drag` 各加 `cgevent-pid` / `cgevent-global` 字段
  - **变更文件**：`src/main.rs`（7 处 method 字段添加/重命名），`tests/commands/test_click.sh`（accept 新 method 名）
  - **价值**：agent 能通过 `method` 字段一眼看出本次调用是否走了 PID-targeted 路径；如果看到任意 `*-global`，就是"忘了 `--app`、刚刚干扰了用户"的审计信号
  - **测试**：296/296 全绿（包含 click suite 重新调整后的断言）

- [x] **F2b** 文档化 focus model（0.1d）— **完成 2026-04-27**
  - **做法**：
    - `plugin/skills/computer-pilot/SKILL.md` 新增 `## Focus Model (why --app matters)` 一节，列出 method 字段表 + 已知限制
    - `CLAUDE.md` 第 6 条"Key/Type targeting"重写为"Focus model — `--app` and PID-targeted delivery"，详述 `EventSource` RAII / `CGEventPostToPid` / `CGEventKeyboardSetUnicodeString` 三件套，给未来贡献者一份清晰的工作模型
    - `cu type` 描述同步更新（不再是 clipboard paste，是 Unicode CGEvent）
  - **价值**：agent 和后续贡献者都能从单一文档源理解"--app 给的是非打扰保证"这件事

- [~] **E3** "不抢焦"自动化测试 — **已实现（2026-04-27）**
  - **实现位置**：`tests/commands/verify_no_disruption.sh`（11 个断言：cursor location + frontmost app + clipboard sentinel）
  - **覆盖**：B1 (click)、B2 (type/key)、B3 (set-value)、B5 (scroll/hover/drag) 全部跑过；含一组反向对照（无 `--app` 时光标确实 warp）
  - **未集成进 run_all.sh 的原因**：需要 5 秒级别的等待 + 干净的桌面状态，会让常规测试套件慢很多。维持手动跑（已在 README 提及）
  - **后续**：Sprint 2 做 D1 多显示器时再考虑是否要把这个 suite 也常驻

- [x] **F1** README "Why cu doesn't disrupt your workflow" 对比表（0.3d）— **完成 2026-04-27**
  - **做法**：README 在原有 `## Why cu?` 后新增 `## Why cu doesn't disrupt your workflow` 一节，含两张表 ——
    1. cu vs Codex CU vs Anthropic CUA vs kagete 在 cursor / frontmost / clipboard / IME / 感知层 / AX 链 / method 审计 七维对比
    2. method 字段全表（ax-action 最佳 / `*-pid` 非打扰 / `*-global` 打扰）
    + 解释 `CGEventPostToPid` + `CGEventKeyboardSetUnicodeString` 双机制；末尾标注 drag/hover 不可避免的光标移动 + sandbox app fallback
  - **价值**：把"我们和 Codex CU 的差距 vs 优势"用一张能直接 share 的表说清楚

---

## Sprint 2 — VLM 桥梁 + CLI 工程力 + 闭环精度（目标 6 天）

> **战略定位**：cu 不是要变成 VLM agent，是要做"已经有视觉的 agent 用得最顺手的 macOS 控制 CLI"。所以 Sprint 2 的重心是 **VLM-friendly 桥梁工具**（A 系列）+ **agent 友好的 CLI 工程力**（G 系列），其次才是闭环精度（B/C/D）。
>
> **完成验收**：① VLM agent 能"看图 → 点 ref"（标注截图 + 像素到 ref 反查）；② agent 调 cu 不依赖 SKILL.md 也能选对工具（per-cmd 选择规则内联）；③ 多步任务不再因为 ref 重排或多显示器失败。

### Sprint 2 完结（18/18）— A 系列 + G1–G4 + B6 + C3 + C4 + D1 + D6 + D7 + F3

- [x] **A1** `cu find` 谓词查询命令 — **完成 2026-04-27**（详见下文 A1 写法块）
- [x] **C1** `cu snapshot --diff` 跨快照 diff — **完成 2026-04-27**（详见下文 C1 写法块）
- [x] **A3** `cu snapshot --annotated` 标注截图 — **完成 2026-04-27**（详见下文 A3 写法块）
- [x] **A8** `cu nearest <x> <y>` 像素 → ref 反查 — **完成 2026-04-27**（详见下文 A8 写法块）
- [x] **A9** `cu screenshot --region` 区域截图 — **完成 2026-04-27**（详见下文 A9 写法块）
- [x] **A10** `cu snapshot --with-screenshot` 树+图融合 — **完成 2026-04-27**（详见下文 A10 写法块）
- [x] **A11** `cu observe-region <x> <y> <w> <h>` 区域候选 ref — **完成 2026-04-27**（详见下文 A11 写法块）
- [x] **G1** 类目化 `cu help` — **完成 2026-04-27**（详见下文 G1 写法块）
- [x] **G2** `cu examples [topic]` 内置 recipe 库 — **完成 2026-04-27**（详见下文 G2 写法块）

### A 系列（VLM 桥梁）— 给已经有视觉的 agent 配最顺手的手

- [x] **A3** 截图叠加 ref 编号（1d）— **完成 2026-04-27**
  - **做法**：`cu snapshot --annotated [--output path]` 复用 `screenshot::find_window` 拿 window_id，调 `CGWindowListCreateImage` 截窗，然后用 **CoreGraphics + CoreText FFI** 在 CGBitmapContext 上重绘：先把截图画进 context，再为每个元素画红色边框 + 红底白字 ref 标签（Helvetica-Bold 14pt × scale）。Retina 自动处理（scale = `image_w / window.width`）。CG y 轴是 bottom-up，每个矩形手动翻转 y。零新依赖（CT 是系统 framework）
  - **变更文件**：`src/screenshot.rs`（+200 行：CG/CT FFI、`Annotation` struct、`annotate_window`、`build_text_line`），`src/main.rs`（`Cmd::Snapshot` 加 `--annotated` / `--output`、`cmd_snapshot` 分流），`tests/commands/test_annotated.sh`（新建，17 个断言）
  - **JSON 输出**：在 snapshot result 上附 `annotated_screenshot: <path>` 和 `image_scale: <ratio>` 两个字段；image_scale 让 VLM 知道像素 ↔ 屏幕坐标转换系数（Retina 是 2.0）
  - **参考已落实**：[ghost-os `Perception/Annotate.swift`](https://github.com/ghostwright/ghost-os) —— 同样思路（红框+数字标签），但我们用纯 CG + CT FFI 实现而不引入图像库
  - **价值**：单一最高 leverage 的 VLM-friendly 特性。Codex CU / Anthropic CUA 都没有"AX ref 可视化"这条路 —— cu 在这条路上有真正独有的优势
  - **验证**：
    1. ✅ 实测 Finder snapshot 30 元素 → 1800x1200 PNG，红框 + 数字清晰可读
    2. ✅ Retina scale=2.0 自动检测（image_w 1800 / window.width 900 = 2.0）
    3. ✅ 默认输出路径 `/tmp/cu-annotated-<ts>.png`，--output 可覆盖
    4. ✅ 与 plain snapshot 共存（plain 不写图、不加字段）
    5. ✅ 与 --diff / --limit 等其他 flag 正交
    6. ✅ Human mode 额外打印 `Annotated screenshot: <path>` 一行
  - **测试**：310/312 命令测试（+17 新增；2 skipped 仍是 A6 modal 与 C1 / A3 无关）
  - **后续可做**：① label 在密集区域可能重叠，未来加 collision-aware 偏移；② 现在所有元素都画框，未来支持 `--annotate-only role=button,row` 选择性标注；③ 颜色按 role 分（button 红 / textfield 蓝 / row 绿）让 VLM 更快定位

- [x] **A8** `cu nearest <x> <y>` 像素 → ref 反向解析（0.2d）— **完成 2026-04-27**
  - **做法**：复用 `ax::snapshot()` 拿元素列表，对每个元素算"点到矩形最近点的欧氏距离"（点在矩形内则距离 0）。返回 `match` 含 ref/role/title/value/坐标/distance/inside；可选 `--max-distance N` 过滤；空匹配是 `match:null`（不是错误）
  - **变更文件**：`src/main.rs`（+ `Cmd::Nearest` enum、dispatch、`cmd_nearest` ~85 行），`tests/commands/test_nearest.sh`（新建，18 个断言）
  - **API**：`cu nearest 480 320 --app X [--limit 200] [--max-distance 50]` → `{"match":{"ref":12,"role":"button","distance":0.0,"inside":true,...}, "query":{"x":480,"y":320}}`
  - **价值**：闭合 VLM-cu 桥梁的另一半。A3 是"看图选 ref"；A8 是"VLM 已经定坐标 → cu 翻 ref"。两者组合让 VLM 既能基于"看图找标签"也能基于"看图给绝对坐标"工作
  - **验证**：
    1. ✅ 点在元素内 → 返回该元素，distance=0，inside=true
    2. ✅ 点在元素外 → 返回最近元素，distance>0，inside=false
    3. ✅ `--max-distance 10` + 远点 → match=null
    4. ✅ nearest 返回的 ref 与同 limit 下 snapshot 一致
    5. ✅ NaN / 不存在 app 走结构化错误
  - **测试**：328/330 命令测试（+18 新增；2 skipped 仍是 A6 modal）

- [x] **A9** `cu screenshot --region` 区域截图（0.2d）— **完成 2026-04-27**
  - **做法**：`screenshot.rs` 新增 `capture_region(x, y, w, h, path)` 用 `CGWindowListCreateImage` 显式 screenBounds 截屏（与 `--full` 共享 listOption）。`main.rs` 加 `--region` flag、新建 `parse_region` helper 接受 `"x,y WxH"` / `"x,y,w,h"` / `"x y w h"` 多种格式。坐标在 point 空间，与 snapshot element 坐标一致。错误处理：非数字 / 4 个组件不齐 / 零或负尺寸都走结构化错误
  - **变更文件**：`src/screenshot.rs`（+30 行 capture_region），`src/main.rs`（`Cmd::Screenshot` 加 region 字段、`cmd_screenshot` 优先分流到 region、`parse_region` helper 21 行），`tests/commands/test_screenshot.sh`（+13 个新断言，覆盖 region success + Retina 缩放 + 大小比对 + 4 个错误路径）
  - **JSON 输出**：`{ok, path, mode:"region", offset_x, offset_y, width, height}` —— offset_x/y 让 agent 把 image 像素映射回屏幕坐标
  - **价值**：实测 300×200 point 区域 = 600×400 px PNG = **85KB（vs 全窗口 471KB，5.5× 小）**。VLM 验证 "按钮变灰了吗"、"modal 消失了吗" 等小问题再也不需要看全屏
  - **验证**：
    1. ✅ 4 种格式（带空格、带逗号、带 x、混合）全部解析成功
    2. ✅ Retina 自动 ×2（PNG 像素 = region 点 × scale）
    3. ✅ Region 文件严格小于全窗口文件（85KB < 471KB）
    4. ✅ 非数字 / 4 个组件不齐 / 0×0 / 负尺寸都返回结构化 CuError
    5. ✅ 与 --app / --full / 默认路径其他 mode 正交（region 优先级最高）
  - **测试**：344/346 命令测试（+13 新增；2 skipped 仍是 A6 modal）

- [x] **A10** `cu snapshot --with-screenshot` 融合输出（0.3d）— **完成 2026-04-27**
  - **做法**：`screenshot.rs` 加 `capture_window_with_scale(window, path)` 复用 raw capture 的 image，直接读 `CGImageGetWidth` 算 scale 再 save。`main.rs::cmd_snapshot` 加 `--with-screenshot` flag —— 当 set 且 `--annotated` 没 set 时调 `capture_window_with_scale` 并把 `screenshot` + `image_scale` 字段附到 JSON。两个 flag 同时给：annotated 优先（已经包含图，不再写 plain）。`--diff` 路径里也接线了：first-call 和 warm 两条都附图 —— 让 VLM 的"看 diff 找改动 + 看图验证"组合工作流可用
  - **变更文件**：`src/screenshot.rs`（+25 行 capture_window_with_scale），`src/main.rs`（`Cmd::Snapshot` 加 with_screenshot 字段、cmd_snapshot 加 plain_screenshot 分支、3 个 emission 路径都加图字段），`tests/commands/test_snapshot_with_screenshot.sh`（新建，24 个断言）
  - **JSON 字段约定**：plain 用 `screenshot` + `image_scale`；annotated 用 `annotated_screenshot` + `image_scale`；同时给两个 flag 时 annotated 字段在、screenshot 字段缺
  - **价值**：保证树和图在**同一 UI 瞬间**采集 —— 避免两次 `cu` 调用之间 UI 漂移导致的 ref 错位。结合 `--diff` 后，VLM 一次调用拿到"diff 哪些元素 + 现在的图"，闭环精度显著提升
  - **验证**：
    1. ✅ plain `--with-screenshot` 返回 `screenshot` + `image_scale`，无 `annotated_screenshot`
    2. ✅ 默认输出 `/tmp/cu-snapshot-<ts>.png`，`--output` 可覆盖
    3. ✅ 不带 flag 的 plain snapshot 不含图字段
    4. ✅ `--annotated` + `--with-screenshot` 同时给 → annotated wins
    5. ✅ `--diff` + `--with-screenshot` 在 first-call 和 warm 两条路径下都附图
    6. ✅ Human mode 多打印 `Screenshot: <path>` 一行
  - **测试**：368/370 命令测试（+24 新增；2 skipped 仍是 A6 modal）

- [x] **A11** `cu observe-region <x> <y> <w> <h>` 区域元素查询（0.3d）— **完成 2026-04-27**
  - **做法**：复用 `ax::snapshot()` 拿元素列表，filter 在 `main.rs::cmd_observe_region` 里做。三种成员关系（`--mode`）：
    - `intersect`（默认）：bbox 与 region 有任何重叠
    - `center`：元素中心点落在 region 内（过滤掉大容器噪声）
    - `inside`：bbox 完全在 region 内（最严）
  - **变更文件**：`src/main.rs`（+ `Cmd::ObserveRegion` enum、dispatch、`cmd_observe_region` ~85 行），`tests/commands/test_observe_region.sh`（新建，22 个断言含 mode 不变量检查）
  - **JSON 输出**：`{ok, app, region:{x,y,w,h}, mode, matches:[...], count, scanned, truncated}`，与 `find` shape 一致便于 jq pipeline
  - **价值**：补全"VLM 视觉感知 → cu 结构化候选"的最后一种粒度。A8 是单点反查（最近一个 ref）；A11 是区域反查（所有候选 ref）。两者按场景互补
  - **验证**：
    1. ✅ 实测 Finder 350×200 区域：intersect=92 / center=88 / inside=69（嵌套关系正确）
    2. ✅ 不变量：center 模式下每个 match 的中心点确实在 region 内；inside 模式下每个 match 的 bbox 完全在 region 内
    3. ✅ 区域在屏幕外 → `count:0 ok:true`（不是错误）
    4. ✅ ref 与同 limit 下 snapshot 一致
    5. ✅ 0×0 / 未知 mode 走结构化 CuError
    6. ✅ Human mode：列表 + 空区域时 `No elements in region (...)` 提示
  - **测试**：390/392 命令测试（+22 新增；2 skipped 仍是 A6 modal）

### G 系列（CLI 工程力）— agent 直接用 CLI 时的最佳实践

- [x] **G1** 顶层 `cu help` 类目化（0.3d）— **完成 2026-04-27**
  - **做法**：用 clap 的 `before_help` 在所有 help 路径（`cu` 无参 / `cu -h` / `cu --help`）顶部注入"COMMANDS BY CATEGORY"块，4 组分类（Discover / Observe / Act / Script & System）共 22 个命令各占一行；保留 `long_about` 的 workflow 叙事 + clap 自动生成的 flat command 列表 → 三层结构（类目快读 → 工作流叙事 → 详细命令表）。同时把"WORKFLOW FOR VLM AGENTS"加进 long_about（A3+A8+A11 的标准用法）
  - **变更文件**：`src/main.rs`（Cli 加 `before_help`、`long_about` 加 VLM workflow 段落），`tests/commands/test_help.sh`（新建，29 个断言：3 个 help 路径 × 类目可见 + flat 列表完整性 + workflow 叙事 + subcmd help 仍可用）；CLAUDE.md / README.md 命令计数 20 → 22（之前漏算 set-value/perform）
  - **为何不砍命令而是分类**：见对话 —— cu 的对标对象是 `gh` / `kubectl` / `aws`（多命令 Unix CLI），不是 Anthropic CUA / Codex CU 的"模型直 tool call"范式。22 个命令在 peer set 里完全合理；agent 友好的关键是**discovery + selection** 清晰
  - **验证**：
    1. ✅ `cu`（无参）/ `cu -h` / `cu --help` 三条路径都首先显示类目化块
    2. ✅ 每个新增 VLM 命令（find/nearest/observe-region）都在分类里
    3. ✅ 所有 22 个命令仍出现在 clap 自动生成的 flat 列表
    4. ✅ `--help` 仍包含 long_about 完整工作流叙事
    5. ✅ `cu <subcmd> --help` 不受影响
  - **测试**：418/421 命令测试（+29 新增；2 skipped 仍是 A6 modal；1 拙时 wait 抖动 isolation 重跑通过）

- [x] **G2** `cu examples [topic]` 内置 recipe 库（0.5d）— **完成 2026-04-27**
  - **做法**：新命令 `cu examples [topic]`，内置 12 个 recipe 作为 `RECIPES: &[(name, summary, body)]` const 数组。无 topic：human 模式列出"name + summary"对齐表，JSON 模式返回 topics 数组；有 topic：打印 3-10 行 working shell snippet（覆盖 launch-app / fill-form / dismiss-modal / read-app-data / wait-for-ui / vlm-click-by-image / vlm-coord-to-ref / vlm-region-candidates / diff-after-action / menu-click / region-screenshot / system-pref）；未知 topic 走 CuError + 在 hint 里列出所有合法 topics + suggested_next 指回 `cu examples`
  - **变更文件**：`src/main.rs`（+ `Cmd::Examples` enum + dispatch + RECIPES const + cmd_examples 60 行），`tests/commands/test_examples.sh`（新建，39 个断言：list shape / 12 个 topic 的 recipe 非空 / 内容 grep / 错误结构化 / human 渲染）；CLAUDE.md / README / SKILL.md 命令计数 22 → 23 + 类目化 help 加 examples 进 Discover 类
  - **覆盖 VLM 工作流**：`vlm-click-by-image`（A3 标注截图 → 看图选 ref）、`vlm-coord-to-ref`（A8 像素 → ref）、`vlm-region-candidates`（A11 区域 → 候选 ref）、`region-screenshot`（A9 区域截图省 token）、`diff-after-action`（C1 cheap re-snapshot）—— A 系列每个 VLM 桥梁工具都有对应 recipe
  - **价值**：agent 卡住时一行命令 `cu examples dismiss-modal` 拿到可直接 copy 的 working example，不需要读 SKILL.md 全文。每个 recipe < 10 行，total recipe library 内嵌在 binary，零额外文件
  - **验证**：
    1. ✅ 12 个 topic 全部返回 `ok:true` + 非空 recipe
    2. ✅ 关键 recipe 内容正确（launch-app 含 cmd+space、vlm-click 含 --annotated 等）
    3. ✅ 未知 topic 返回 CuError，hint 列出全部 topics，suggested_next 指回 `cu examples`
    4. ✅ Human mode 输出对齐表 + 单 topic 的 `# topic — summary` 头
    5. ✅ JSON list 形式 / JSON detail 形式 / Human 列表 / Human 单 topic 四种渲染都正确
  - **测试**：+39 新增；命令计数从 23 命令开始（之前漏算 set-value/perform，已在 G1 修正到 22；G2 加 examples 到 23）

- [x] **G3** `cu find --first --raw` 直接输出 ref 整数（5 分钟）✅ 2026-04-27
  - **做法**：`--raw` flag 让 `cu find` stdout 只 print bare ref 整数（每行一个），免 jq；no-match 退出 1 + 无输出
  - **价值**：`cu click $(cu find --app X --role button --title-equals Save --first --raw)` 一行成
  - **测试**：`tests/commands/test_find.sh` +4 assertions（`--first --raw` 单整数、多行整数、no-match 退 1、pipe-friendly）

- [x] **G4** 每个 subcommand 的 after_help 加"PREFER:"块（0.3d）✅ 2026-04-27
  - **做法**：在 7 个有重叠用法的 subcommand 的 `after_help` 加 `PREFER:` 块（agent 跑 `cu <cmd> --help` 直接看到选择指引）
    - `cu set-value` → prefer over `cu type` for AX textfields/textareas/comboboxes
    - `cu type` → prefer `cu set-value` for AX textfield；type 用于 non-AX (Electron) 或已有焦点的输入流
    - `cu perform` → 一般 case 用 `cu click`，只在需要非 AXPress 动作时用 perform
    - `cu tell` → 优先于 click/type 用于 scriptable apps（`cu apps` 的 S flag）
    - `cu find` → 优先于 `cu snapshot | grep`
    - `cu nearest` → VLM 视觉坐标 → ref 反查
    - `cu observe-region` → VLM 圈定区域后枚举候选 ref
  - **测试**：`tests/commands/test_help.sh` +7 assertions（每个命令 --help 含 `^PREFER:` 块）

### 原 Sprint 2 任务（闭环精度）— 优先级下移到 A/G 之后

- [x] **B6** AX 提窗替代全局 activate（0.5d）✅ 2026-04-27
  - **做法**：`src/ax.rs::raise_window(pid)` — 取 `AXMainWindow`/`AXFocusedWindow`，`AXMain=true` + `AXRaise`，零 AppleScript。`cu window focus` 默认走此路径，AX 失败时 fallback 到 System Events 旧行为
  - **响应**：返回 `method: "ax-raise"`（成功）/ `"applescript-frontmost"`（fallback）
  - **测试**：`tests/commands/test_window.sh` +1 assertion（`focus uses method=ax-raise`）

- [x] **C3** `cu wait` 高级条件（1d）✅ 2026-04-27
  - **做法**：`wait::Condition` 增加 `NewWindow` / `Modal` / `FocusedChanged` 三个 variant，主循环首次 poll 捕获 baseline。`NewWindow` 直接调用 `ax::window_count(pid)`（查 `AXWindows` 数组），不依赖 snapshot.elements（只走 focused window）；`Modal` 看 `snap.modal`；`FocusedChanged` 比较 `snap.focused.ref` 与 baseline
  - **CLI flags**：`--new-window` / `--modal` / `--focused-changed`（与现有 `--text` / `--ref` / `--gone` 互斥）
  - **测试**：`tests/commands/test_wait_advanced.sh` 8 assertions（错误路径、超时计时、动态打开新窗口被检测到 ~1.2s）

- [x] **D1** 多显示器坐标系一等处理（1d）✅ 2026-04-27
  - **做法**：新建 `src/display.rs` — `CGGetActiveDisplayList` + `CGDisplayBounds` + `CGMainDisplayID`，返回 `Vec<DisplayInfo{id, main, x, y, width, height}>`。`cu snapshot` 在所有 JSON 输出路径（普通 / `--diff` first / `--diff` warm）顶层注入 `displays` 数组，agent 可基于 element 的 (x,y) 自行解析归属屏幕
  - **接口**：`display::list()` / `display::display_for_point(x, y, &displays)`（后者保留给后续 mouse 验证）
  - **测试**：`tests/commands/test_displays.sh` 7 assertions（数组 shape、exactly-one-main、diff 路径、主屏 bounds 合理性）

- [x] **D6** App 启动等待原语（0.5d）✅ 2026-04-27
  - **做法**：`cu launch <name|bundleId>` 经 `open -a` / `open -b` 拉起，默认轮询 100ms 直到 AX 报告 main/focused window；`--no-wait` 跳过等待，`--timeout` 超时退出 1。bundle id 经 `system::resolve_by_bundle_id` (System Events `whose bundle identifier is`) 反查到 `(pid, name)`
  - **响应**：`{ok, app, pid, ready_in_ms, waited, window:{x,y,width,height}}`
  - **测试**：`tests/commands/test_launch.sh` 16 assertions（name 路径、bundle id 路径、warm/cold、no-wait、error、human mode）

- [x] **D7** 单次 AXObserver wait 替代固定 500ms（0.5d，新增）✅ 2026-04-27
  - **做法**：新建 `src/observer.rs`（~180 行 FFI），`maybe_attach_snapshot` 入口改为 `observer::wait_for_settle(pid, POST_ACTION_DELAY_MS)`：`AXObserverCreate` → 订阅 `AXValueChanged` / `AXFocusedUIElementChanged` / `AXMainWindowChanged` / `AXSelectedChildrenChanged` → `CFRunLoopRunInMode` 50ms 切片轮询，首个 notification 触发即返回；超时 fall back 到 sleep；observer 仅活在单次调用范围内（无 daemon）
  - **响应**：所有动作响应新增 `settle_ms` 字段，记录实际等待时长
  - **价值**：典型场景 settle_ms ≈ 50-200ms（vs 之前固定 500ms），同时上限仍是 500ms 防止失控
  - **测试**：`tests/commands/test_settle.sh` 6 assertions（present、integer、≤cap、3 次采样最大值、--no-snapshot 时不附加）

- [x] **C4** 动作 method 加 confidence + advice 字段（0.2d）✅ 2026-04-27
  - **做法**：`src/main.rs::method_meta(method)` → `(confidence, advice)` 表，由 `annotate_method` 在 `maybe_attach_snapshot` 入口统一注入到所有动作响应。`ax-action`/`ax-set-value`/`ax-perform`/`*-pid` = high；`ocr-text-pid` = medium + verify advice；`*-global` = low + "pass --app" advice
  - **测试**：`tests/commands/test_method_meta.sh` +8 assertions（key --app=high/no-advice、no-app=low/has-advice、set-value=ax-set-value/high）

- [x] **F3** SKILL.md 升级为 cookbook + 决策树（0.5d）✅ 2026-04-27
  - **做法**：① 顶部新增 Decision Tree（goal-shaped tree → 命令）+ Hard Rules；② 末尾新增 10-recipe Cookbook（launch/scriptable read/set-value/find-by-label/VLM-coord-click/observe-region/region-screenshot/wait-conditions/snapshot-diff/defaults）；③ 引用计数升到 24 命令，Output Format 新增 method/confidence/advice/settle_ms/displays 字段说明
  - **价值**：agent 一眼就能定位 "我现在该用哪个命令"，减少 prompt 中的反复试错

- [x] **A1** `cu find --role X --title-contains Y` 命令（0.5d）— **完成 2026-04-27**
  - **做法**：复用 `ax::snapshot()` 已有的 walker，filter 在 `main.rs::cmd_find` 里做。零新 walker 代码，returned ref 与同 `--limit` 下 snapshot 完全一致 → 直接 `cu click <ref>` 可用。filters 全部 AND：`--role`（按 normalized lowercase 匹配，如 `button` / `row`）、`--title-contains`（大小写无关子串）、`--title-equals`（精确）、`--value-contains`（大小写无关子串）。`--first` 改返回 `.match` 单 object（适合 `... | jq -r .match.ref | xargs cu click`）。空结果是 `ok:true count:0` —— 不是错误。0 filter 时返回结构化错误（CuError + suggested_next）
  - **变更文件**：`src/main.rs`（+ `Cmd::Find` enum、dispatch、`cmd_find` ~100 行），`tests/commands/test_find.sh`（新建，24 个断言），SKILL.md / README.md / CLAUDE.md（命令计数 17 → 18 + 新增"Targeted query"段落）
  - **参考已落实**：[kagete `find` 命令](https://github.com/ringotypowriter/kagete)
  - **验证**：
    1. ✅ `cu find --app Finder --role row` 返回所有 row，scanned/truncated 字段就位
    2. ✅ `cu find --first` 返回单个 `.match` object，empty 时为 null
    3. ✅ AND 过滤正确收窄（role=row + title-contains 严格 ≤ role=row 单独）
    4. ✅ find 返回的 ref + 坐标与同 limit 下 snapshot 完全一致
    5. ✅ 大小写不敏感（lowercase / UPPERCASE 同 count）
    6. ✅ 0 filter / 不存在 app 走结构化错误
  - **测试**：320/320 命令测试全绿（296 原有 + 24 新增）

- [x] **C1** Diff snapshot（0.5d）— **完成 2026-04-27**
  - **做法**：新增 `cu snapshot --diff`，独立 standalone 命令（不动作命令注入，保持改动小、可组合）。新建 `src/diff.rs` —— 缓存路径 `/tmp/cu-snapshot-cache/<pid>.json`；元素 identity = `(role, round(x), round(y))`，robust 于 ref 重排、敏感于窗口移动；content_changed 判断 = title / value / size 任一变化（width/height tolerance 0.5px）。第一次调用没缓存 → 返回完整 snapshot + `first_snapshot:true`，agent 知道下一次起就有 diff。`Element` 加 `Deserialize + Clone` 支持反序列化
  - **变更文件**：`src/diff.rs`（新建 92 行），`src/ax.rs`（Element 加 derive），`src/main.rs`（`Cmd::Snapshot` 加 `--diff` flag、`cmd_snapshot` 分流、`print_diff_human` 用 `+ ~ -` 标注），`tests/commands/test_snapshot_diff.sh`（新建，21 个断言），SKILL.md / README.md（用法段落）
  - **参考已落实**：自实现 —— 没有开源同类做这事，是 cu 又一项行业首创
  - **验证**：
    1. ✅ 第一次调用：`first_snapshot:true` + 完整 elements
    2. ✅ 二次调用无 UI 变化：`+0 ~0 -0`，unchanged_count = 总元素数
    3. ✅ set-value 后：精准抓到 textarea 一项 `~`，其他 19 项不变
    4. ✅ Cache 文件正确写入 `/tmp/cu-snapshot-cache/<pid>.json`
    5. ✅ Human mode 用 `+ [ref] role`、`~ [ref] role`、`- [ref] (removed)` + Summary 行
    6. ✅ `--diff` 与 plain snapshot 共存（plain 不影响 cache 一致性）
  - **测试**：293/295 命令测试（2 skipped 是 A6 modal trigger 受 macOS iCloud auto-save 抑制 —— 已改为 _skip 而非 fail，与 C1 无关）
  - **后续可做**：动作命令的 `--diff-snapshot` flag（取代默认的 full snapshot 注入）—— 等真实 agent 用了 `cu snapshot --diff` 一段时间后，根据反馈再决定是否进一步集成
  - **已知局限**：窗口移动会让所有元素 identity 变化 → 全部 added+removed。这是 identity-by-position 的固有取舍；agent 在多步流程中应避免移动窗口（或在窗口移动后第一次 diff 把 first_snapshot 视为预期）

---

## Sprint 3 — 长期能力 + 可观测性（5 天+）

- [x] **A2** axPath 稳定 selector（2d）✅ 2026-04-27
  - **做法**：`src/ax.rs` walker 在 DFS 时给每个 Element 计算 axPath 字段，格式 `Role[Title]:N/Role[Title]/...`（`[Title]` 可选，`:N` 是同级同段重复时的 0-indexed 占位）。CLI 给 `cu click` / `cu set-value` / `cu perform` 加 `--ax-path` flag，分别走 `ax::resolve_by_ax_path` / `ax::ax_set_value_by_path` / `ax::ax_perform_by_path` 三个 top-down resolver——重新走 AX 树按段匹配，不依赖 ref 编号
  - **格式约定**：title 中的 `/` `[` `]` 会被替换成 `_`；title 超过 60 字符截断 + `…`；segment 默认 `:0` 略写
  - **价值**：多步流程不再被 ref 重排打断。一次 snapshot 抓所有要用的 axPath，后面步骤直接 `--ax-path` 用，跨 snapshot 稳定
  - **测试**：`tests/commands/test_ax_path.sh` 11 assertions（每元素都有 axPath、:N 出现且全唯一、coord round-trip 跟 snapshot 一致、错误路径、set-value 拒读只元素、perform 缺 selector 报错）

- ~~**A3** 截图叠加 ref 编号~~ — **已移到 Sprint 2 第一位**

- [x] **D8** AX bridge 预热（0.3d）✅ 2026-04-27
  - **做法**：`cmd_launch` 在窗口出现后追加 `ax::snapshot(pid, &name, 5)`，响应附 `warmup_ms` 字段；新增 `cu warm <app>` 让用户自己开的应用也能手动预热
  - **背景**：TextEdit / Mail 等首次 AX walk 有 200–500ms 冷启延迟，影响第一条 click/snapshot 的响应时长
  - **测试**：`tests/commands/test_warm.sh` 8 assertions + `test_launch.sh` 新增 `warmup_ms` 断言

- [x] **B7** 失败诊断 `cu why`（0.5d）✅ 2026-04-27
  - **做法**：新增 `ax::inspect_ref(pid, ref_id)` 走 AX 树取出 AXEnabled / AXFocused / AXSubrole / 支持的 actions；`cu why <ref> --app <name>` 拼装结构化 `{ found, element, checks, advice }` —— check 包括 in_snapshot / in_window_bounds / click_supported / modal_present，advice 文本覆盖 modal 阻塞 / disabled / 无 AXPress / sandbox 沙盒等典型失败原因
  - **价值**：click 返回 `ok:false`（或返回 ok 但 UI 没变）后，agent 直接调一次 why 就能知道 "ref 不存在 / 元素 disabled / 不支持 AXPress 应该用 perform / sandbox 应用要换路径" 等具体原因，少走探索性 grep
  - **测试**：`tests/commands/test_why.sh` 15 assertions（found / 缺失 ref / 非运行 app / human mode）

- [ ] **A5** Chrome CDP bridge（3d）
  - **做法**：检测目标是 Chrome/Edge/Electron 时尝试连 9222 端口
  - **参考**：[ghost-os `Vision/CDPBridge.swift`](https://github.com/ghostwright/ghost-os)
  - **取舍**：用户必须手动启用 Chrome 的 debug port，体验不如 Codex CU；优先级中后

- [ ] **E1** macOSWorld baseline 跑通 + 发布（1d）
  - **做法**：跑 GPT-5.4 / Claude Opus 4.7 / Sonnet 4.6 三档基线，发布到 README
  - **现有基础**：`tests/macosworld/` + `tests/agent/caliper_records.json`（已 untracked）+ `tests/agent/caliper_report.py`（已 untracked）
  - **完成标准**：README 含 baseline 表 + 链接到 reproducible 脚本

- [ ] **E2** Regression dashboard（1d）
  - **做法**：每次发版自动跑 macOSWorld 子集，结果归档，diff 上次结果

---

## 已实现的部分（与 Codex CU 工艺对比）

> 不只是清单，每项含与 Codex CU 的细节差异，作为复盘和持续优化的依据。

### ✅ AX 树为主 + 截图为辅（A0）
- **cu 实现**：`src/ax.rs` 919 行 + `src/screenshot.rs` 299 行
- **vs Codex CU**：
  - cu snapshot 是 flat 文本（每行一 ref）；Codex CU 输出缩进树结构 → cu 更省 token，模型选 ref 更快，但层次关系信息少
  - cu 强制 `--limit`（默认 50）；Codex CU 似乎自适应裁剪 → cu 更可控，但有时需要多次 snapshot
  - cu 同时附 PNG 字段；Codex CU 默认不带 → cu 更"一站式"，但 token 更贵

### ✅ AX 动作链（B0）
- **cu 实现**：`src/ax.rs:553` `try_ax_actions`，**15 步**（AXPress → AXConfirm → AXOpen → AXPick → AXShowAlternateUI → child action → 复选框 toggle → AXSelected → 父行选中 → focus+press → 祖先 press → CGEvent）
- **vs Codex CU / 开源同类**：
  - 比 [iFurySt/open-codex-computer-use](https://github.com/iFurySt/open-codex-computer-use)（只在 click 时检查 `prettyActions` 试 AXPress）**更细**
  - 与 ghost-os 不在同一抽象层（ghost-os 用 AXorcist 库的高级 PerformActionCommand）
  - **cu 在这一项处于行业领先**

### ✅ 静默窗口截图（A0 子项）
- **cu 实现**：`src/screenshot.rs` 用 `CGWindowListCreateImage`，不需激活
- **vs Codex CU**：Codex CU 用 ScreenCaptureKit（更新 API），效果等价；cu 选 CGWindowList 是因为兼容性更广

### ✅ Auto-snapshot after action（C0）
- **cu 实现**：`maybe_attach_snapshot` 在所有动作命令里调用，含 ~500ms 固定延迟
- **vs Codex CU**：
  - 延迟策略：cu 固定 500ms；Codex CU 推测用 `AXObserverCreate` 监听 UI 变化（更快）
  - 反向 opt-out：cu 提供 `--no-snapshot`，Codex CU 未公开
  - **优化方向**：未来用 AXObserver 替代固定 sleep（优先级中后）

### ✅ Retina / 缩放处理（D2）
- **cu 实现**：`screenshot.rs` 输出 offset_x/offset_y/scale
- **vs Codex CU**：等价

### ✅ OCR 兜底（A0 子项）
- **cu 实现**：`src/ocr.rs` 调 macOS Vision framework via objc2
- **vs Codex CU**：等价；同类开源里只有 [axon](https://github.com/gxcsoccer/axon) 也用了 Vision

### ✅ 三层 hybrid 架构（AppleScript → AX → screenshot）
- **cu 实现**：`cu tell` / `cu sdef` / `cu snapshot` / `cu click` / `cu screenshot` 分层暴露
- **vs Codex CU**：Codex CU 不强调 AppleScript 这一层（虽然内部疑似有用）；cu 把 AppleScript 作为 scriptable app 的一等通道是独到选择
- **vs 开源同类**：[axon](https://github.com/gxcsoccer/axon) 是同款三层架构（Swift 实现）

### ✅ 单二进制零运行时依赖
- **cu 实现**：纯 Rust + 系统 framework FFI
- **vs Codex CU**：Codex CU 是 macOS app + cloud；cu 是 CLI，分发更轻
- **vs 开源同类**：所有 Swift 项目都需 swift build；Node 项目需 cliclick + pyobjc

### ✅ 完整测试基建
- **cu 实现**：258 命令测试 (`tests/commands/run_all.sh`) + agent E2E (`tests/agent/run.py`) + macOSWorld (`tests/macosworld/`)
- **vs 开源同类**：kagete / axon / ghost-os 测试覆盖都更弱；cu 反而最齐全

---

## 明确舍弃（Out of Scope）

| 项 | Codex CU 是否做了 | 我们不做的原因 |
|---|---|---|
| 软光标 overlay（虚拟光标，user 看到 agent 操作但不被打断）| ✅ 标志性 UX | 需要 SwiftUI/AppKit helper 进程，违反"零运行时依赖"原则 |
| MCP server 模式 | ❌（不是 MCP）| 违反 CLAUDE.md 明确的 "CLI only, no MCP" 原则 |
| ghost-os 风格 record/replay 自学习 recipe | ❌ | 超出当前产品范围，需单独产品决策 |
| 视觉模型 fallback（cu 内置 VLM 调用） | 🟡 GPT-5.4 自带，模型与工具一体化 | 我们的 agent 自己有 vision（Claude / GPT），cu 是被复用的"手"，不应再调一次远程 VLM |
| 长生命周期 daemon + AXObserver 推送 | ✅ 内部架构 | 违反"单二进制 CLI"哲学；用 D7（单次 AXObserver wait）拿走 80% 价值，剩余的 daemon 收益不值复杂度 |
| 模型与工具一体化训练（co-trained）| ✅ Codex CU 核心壁垒 | 我们是 model-agnostic 工具，刻意不绑定特定模型 —— 这是 cu 相对 Codex CU 唯一的护城河 |

> 关键洞察：cu 与 Codex CU 在战略上**根本不同**。Codex CU 走"模型+工具一体化"封闭产品；cu 走"任何 agent + shell 都能用、零集成成本"开放工具。两者各自的最优解很多时候是相反的（daemon vs CLI、内置 VLM vs 把 vision 留给 agent、训练共生 vs model-agnostic）。Sprint 2 的设计原则是**在不放弃 cu 战略前提下，把 VLM agent 用 cu 时的体验做到最好**。

---

## 主要参考项目

| 项目 | 星数 | 语言 | 借鉴模块 | 主要价值 |
|---|---|---|---|---|
| [iFurySt/open-codex-computer-use](https://github.com/iFurySt/open-codex-computer-use) | 555 | Swift | `InputSimulation.swift` / `ComputerUseService.swift` / `AccessibilitySnapshot.swift` | Codex CU 最直接的开源复刻；B1/B2/B5/B6/A4/D1 主样板 |
| [ringotypowriter/kagete](https://github.com/ringotypowriter/kagete) | 2 | Swift | `Input.swift` / `AXRaise.swift` / `find` / `set-value` / `action` 命令 | CLI 命令设计模型，axPath 路线 |
| [ghostwright/ghost-os](https://github.com/ghostwright/ghost-os) | 1412 | Swift | `Annotate.swift` / `CDPBridge.swift` / `ToolResult` | 截图标注、Chrome 增强、富错误返回 |
| [gxcsoccer/axon](https://github.com/gxcsoccer/axon) | 0 | Swift | `perform` 命令设计 | B4 命令形态 |
| [bradthebeeble/mcp-macos-cua](https://github.com/bradthebeeble/mcp-macos-cua) | 0 | Node.js | 产品化 onboarding | `/cua` skill 自动 permission bootstrap 思路 |

---

## 进度总览

| Sprint | 状态 | 起 / 止 | 备注 |
|---|---|---|---|
| Sprint 1 — 不抢焦 + 工具暴露 | **完成** | 2026-04-27 | 10/10 任务（含 F2 关闭，F2a + F2b + E3 等价完成）|
| Sprint 2 — VLM 桥梁 + CLI 工程力 + 闭环 | ✅ 完结 | 2026-04-27 | 18/18：A 系列 (5) + A1/C1 + G1–G4 + B6/C3/C4/D1/D6/D7/F3；24 命令、479 测试 |
| Sprint 3 — 长期能力 | 进行中 | 2026-04-27 — | A2 axPath + D8 AX warmup + B7 cu why 完成；A5 / E1 / E2 待启动 |
