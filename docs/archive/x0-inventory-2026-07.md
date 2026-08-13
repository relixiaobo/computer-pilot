# X0 — 平行实现清点（2026-07-31，r6 修订）

> **Archived 2026-08-13.** This inventory describes `main` at `ff5088b`
> (`v0.8.0`). It is useful provenance, but its line numbers and completeness
> claims are no longer current. Use [`../ROADMAP.md`](../ROADMAP.md) for active
> work and re-inventory the present tree before implementation.

> **目的**：三轮 review 暴露同一个模式 —— 我按"读了主路径 = 读完了"的方式调研，
> 而这个代码库里同一个概念操作普遍存在多份平行实现。
> 每次低估的倍数：ref 编号 1 → 2 → **5**；窗口解析 7 → **5 类机制**；
> axPath 消费者 3 → 6 → 12 → **20**。
>
> **本文是 [`optimization-plan-2026-07.md`](./optimization-plan-2026-07.md) 的事实基础，
> 与计划正文同步维护。** 计划已把本文的全部裁决合入正文 ——
> 不要把两份文档当作"主计划 + 纠偏附录"来共同解释。
>
> **同步规则（r6 新增）**：计划推翻本文任何裁决时，**必须同一次改动同步本文**。
> r6 之前本文滞后过一次（limit 命令名错标、窗口回退范围、`--window 1` 契约三处），
> 而本文又被声明为"事实基础" —— 那正是 r3 已经指出过的"两份文档共同解释"反模式。
>
> **基线**: `main` @ `ff5088b` (v0.8.0)
> **方法**：每个概念用 **3–5 个互相独立的检索角度**交叉验证。
> 只有当所有角度收敛到同一组站点时，才认为枚举完整。每个角度的命令都写在下面，可复核。

## 修订记录

### r6

| 修订 | 原因 |
|---|---|
| 清点 3 的 CLI 默认值表更正命令名，补 `Type`/`Key` 无 limit 一行 | 初稿把 294/330 错标成 `type`/`key`（实为 `Perform`/`SetValue`），并未提及 `Wait`(520)。**行号一直对，命令名错了**，且已传导进计划 v4 |
| 机制 D 裁决从"显式 `--window` 下禁用"改为**无条件禁止** | 默认 Focused 路径在 AX 失败后同样落到 `screenshot.rs:347` |
| 删除"`--window 1` == 默认需写成断言"，改为默认独立定义为 `Focused` | 那是把不受 API 保证的 `AXWindows` 顺序固化成契约 |

### r3

| 修订 | 原因 |
|---|---|
| 清点 4 的角度 2 从"四文件"改为**全仓检索**，新增角度 4（解析侧）；消费者 12 → **20** | 初稿把 `src/ax.rs` 排除在检索范围外，漏掉整套 axPath 解析器族 + `detect_focused`。**X0 自己犯了 X0 要防的错** |
| 新增 [C2 生成器/解析器互逆不变式](#c2-生成器与解析器必须互逆r3-新增的不变式) | 上条连带发现：编码与解码是两套独立实现，无往返测试，而 B2 的合成元素会直接威胁它 |
| 机制 D 的裁决从"两个选项"收敛为**单一禁令** | 初稿提供的"让回退按索引选"违反 `AGENTS.md:228` Principle 1 |
| B2 影响项补"装饰节点按 actions 识别，不按 title" | 初稿只修表达能力，未修规则本身 |

---

## 目录

- [清点 1 — ref 编号](#清点-1--ref-编号)
- [清点 2 — 窗口解析](#清点-2--窗口解析)
- [清点 3 — limit 语义](#清点-3--limit-语义)
- [清点 4 — axPath 生产与消费](#清点-4--axpath-生产与消费)
- [对计划条目的影响](#对计划条目的影响)
- [自查更正](#自查更正)

---

## 清点 1 — ref 编号

### 检索角度

```bash
# 角度 1：计数器增量
grep -n "counter += 1" src/*.rs
# 角度 2：接受计数器的函数签名
grep -n "counter: &mut usize" src/*.rs
# 角度 3：发 ref 的谓词
grep -n "is_included" src/*.rs
# 角度 4：尺寸门槛
grep -n "width > 0.0\|height > 0.0" src/*.rs
```

**四个角度全部收敛到同一组 5 处**（角度 4 另有 3 处在 `screenshot.rs`，是缩放计算，非 ref 逻辑）。
枚举视为完整。

### 完整枚举

| # | 函数 | 增量点 | 入口 | 服务的命令 | 属性读取 | 有 `limit`？ | 记录 `depth_limited`？ |
|---|---|---|---|---|---|---|---|
| 1 | `walk` | `ax.rs:936` | `snapshot` (`ax.rs:2109`) | `snapshot` `state` `find` `nearest` `observe-region` `wait` `why`(前半) 及全部动作前后快照 | **批量** `batch_read` | ✅ | ✅ |
| 2 | `find_and_perform_action` | `ax.rs:1217` | `ax_perform` (`ax.rs:1741`) | `cu perform` | 单属性 | ❌ | ❌ |
| 3 | `find_and_set_value` | `ax.rs:1264` | `ax_set_value` (`ax.rs:1691`) | `cu set-value` | 单属性 | ❌ | ❌ |
| 4 | `find_element_by_ref` | `ax.rs:1308` | `resolve_ref` (`ax.rs:1360`) → `ax_click` / `ax_find_element` | `cu click` | 单属性 | ❌ | ❌ |
| 5 | `find_and_inspect` | `ax.rs:1805` | `inspect_ref` (`ax.rs:1863`) | `cu why`(后半) | 单属性 | ❌ | ❌ |

### 分叉轴（这五份能在哪里不一致）

| 轴 | `walk` | 其余四份 | 后果 |
|---|---|---|---|
| **批量读整体失败** | `values.is_null()` → **跳过本节点、不 +1**，只递归子节点（`ax.rs:888`） | 不走批量，正常 +1 | **该节点之后所有 ref 错位一格** |
| **批量读部分失败** | `batch_string` 检出 error marker 返回 `None`（`ax.rs:793`）→ `AXRole` 拿不到 → 不 +1 | `ax_string` 单独读，可能成功 | 同上，且 `values.is_null()` 的 fallback **不会触发** |
| **`limit` 截断** | 到 `limit` 停 | 无界 | 见清点 3 |
| **深度上限** | `MAX_DEPTH` → 置 `depth_limited` 上报 | `MAX_DEPTH` → 静默返回 `None` | agent 收到 "not found" 而非 "树太深" |

> **第二行是 review r2-F2 指出、v2 计划漏掉的**：v2 的 X1 只处理了整体 null。
> `AXUIElementCopyMultipleAttributeValues(..., options=0)` 会返回**非空数组但个别项是
> error marker**，而 `batch_string`（`ax.rs:783-797`）已经明确在处理这种情况 ——
> 说明这条路径真实存在，不是理论风险。

> **给未来改动的提醒（r7）**：上表的"批量读部分失败"分叉轴对
> **`BATCH_ATTR_NAMES` 的每一个成员**都成立，包括**将来新增的**。
> 计划 B1a 会加入 `AXSelected`/`AXFocused`（服务 Observation 安全门禁），
> B2 会用到 `AXActionNames` —— **任何新增成员都必须同步进计划 X1 的逐属性 fallback 表
> 与故障注入清单**。r7-#1 就是漏了这一步：B1a 加了字段却没同步表，
> 两侧同时读到 error marker 时 `None == None`，安全门禁被自己重新开洞。

### 必须成立的不变式

> 对同一 pid、同一窗口、同一时刻，五份实现对任意 `ref_id` 必须解析到**同一个 AX 元素**。

**当前无任何测试守护这条不变式。**

### 测试为什么必须读 action 路径的身份

r2-F1 指出：`cu why` 的元素身份 `el` 来自它刚跑的 `snap.elements`（实现 #1），
而 `inspection = ax::inspect_ref(pid, ref_id)`（实现 #5）是**另一次独立遍历**，
只取 `actions/enabled/focused/subrole` 合并进结果，**身份从不与 `el` 比对**
（`main.rs:3766-3776`）。

→ 拿 `cu why` 的 axPath 去比 snapshot 的 axPath，等于 snapshot 比 snapshot，测不到分叉。
**等价性测试必须让 action 路径回报它自己解析到的 identity**，这需要新增测试出口。

---

## 清点 2 — 窗口解析

### 检索角度

```bash
grep -n "AXFocusedWindow\|AXMainWindow" src/*.rs      # 角度 1
grep -rn "focused_window_geom" src/*.rs                # 角度 2
grep -n "find_window\|window_id" src/screenshot.rs     # 角度 3
grep -n "AXWindows" src/ax.rs                          # 角度 4
```

### 五类机制

**机制 A —— `AXFocusedWindow → AXMainWindow` 直接读取（8 处）**

| 位置 | 函数 | 路径 |
|---|---|---|
| `ax.rs:421` | `focused_window_geom` | 共享 helper（见机制 B） |
| `ax.rs:1352` | `resolve_ref` | **动作**（click） |
| `ax.rs:1572` | `with_ax_path` | **动作**（ax-path 系） |
| `ax.rs:1684` | `ax_find_element` | **动作** |
| `ax.rs:1734` | `ax_set_value` | **动作** |
| `ax.rs:1857` | `inspect_ref` | 诊断 |
| `ax.rs:1984` | `window_bounds` | **校验**（B4 依赖） |
| `ax.rs:2055` | `snapshot` | 观测 |

**机制 B —— `focused_window_geom` 的下游（3 个消费者）**

| 位置 | 用途 |
|---|---|
| `broker.rs:836` | `publish_observation` —— **写入 Observation 的 window_id** |
| `broker.rs:1828` | `enforce_expected_observation` —— **校验 window_id** |
| `screenshot.rs:332` | `find_window` —— **截图 / 标注 / capture-protection 全部经此** |

> **机制 B 第三行是 v2 计划完全漏掉的（r2-F5）**：`snapshot --with-screenshot`、
> `--annotated`、`cu state` 的图像都走 `screenshot::find_window(pid)` → 焦点窗口。
> B3 只改 AX 侧的话，会产出**"窗口 2 的树 + 窗口 1 的截图"**。

**机制 C —— `AXWindows` 数组 + 索引（3 处）**

| 位置 | 函数 | 用途 |
|---|---|---|
| `ax.rs:465` | `list_windows` | `cu window list` |
| `ax.rs:530` | `window_action` | `cu window move/resize/focus/...` 的 `--window N` |
| `ax.rs:1959` | `window_count` | `cu wait --new-window` |

**这是 `--window <index>` 的现有语义来源。** B3 想复用它，但机制 C 与机制 A
**没有 API 层面的一致性保证**。

实测（`cu window list` 首项 vs `cu snapshot` 的 `window_frame`）：

```
Finder : 3 windows, AXWindows[0] = (139,144 920x464) == AXFocusedWindow ✅
Code   : 4 windows, AXWindows[0] = (0,33 1512x949)   == AXFocusedWindow ✅
Ghostty: 1 window,  AXWindows[0] = (0,33 1512x949)   == AXFocusedWindow ✅
```

三例一致（macOS 通常按前后顺序排列 `AXWindows`），但**这是经验而非契约**。

> **r6 更正**：初稿写"必须写成断言测试" —— **那是把不受保证的顺序固化成 API 契约**。
> `list_windows`（`ax.rs:465`）直接暴露 `AXWindows` 数组顺序；在顺序不同的 app 上
> 该断言必然失败。
>
> 正确做法：**默认独立定义为 `WindowSelector::Focused`**（不是 `Index(1)`），
> `--window N` 严格对应 `cu window list` 第 N 项。两者是不同的选择器，不互为别名，
> 也不断言 N=1 等于默认。

顺带：Finder 3 个窗口、VS Code 4 个 —— **多窗口是常态而非例外**，B3 的价值被低估了。

**机制 D —— CGWindowList 启发式回退**

`screenshot.rs:418` `find_window_with_options`：收集全部 layer-0 且 pid 匹配的窗口，
**按面积取最大**（`screenshot.rs:344-354` 两次调用，先 OnScreenOnly 再放宽）。

这正是 CLAUDE.md「Anti-patterns」第一条禁止的做法，作为 AX 失败时的兜底保留。
→ **B3 加 `--window N` 后，这条回退会静默忽略选择器。**

**裁决（r6 最终）**：**无条件禁止启发式回退**，不分显式/默认路径。

> X0 初稿曾写"要么禁用回退，要么让回退也能按索引选" —— **后者违反项目硬规则**
> （`AGENTS.md:228` Principle 1: "Do **not** pick a window from CGWindowList by
> heuristic (largest area, lowest layer, first match)"）。CGWindowList 的顺序没有任何
> AX 语义，按它的索引选窗口会再次产出"窗口 2 的树 + 另一个窗口的图像"。该选项已删除。

> **r3 的裁决范围划窄了**：它只禁了"显式窗口选择"路径，
> 但**默认 Focused 路径在 AX 失败后同样落到 `screenshot.rs:347`**，
> 同样违反 `AGENTS.md:228`，并让 screenshot/OCR 对错误窗口返回 `ok:true`。
> 这条硬规则没有"仅在显式选择时生效"的版本。

规则（对**所有**路径，含默认 Focused）：
1. window_id **必须**来自 AX 解析出的 `AXWindow`（机制 C）
2. CGWindowList **只允许**按该 id 反查 sharing state（机制 E）
3. AX 给不出 window ID → 返回结构化错误，**不回退**
4. `find_window_with_options`（`screenshot.rs:418`）的"取面积最大 layer-0"**整体删除**

**机制 E —— 按 window_id 反查**

`screenshot.rs:396` `sharing_state_for_window_id` —— 继承上游给的 id，本身不选窗口。

### 必须成立的不变式

> 一次命令内，**AX 树、ref 解析、前后校验快照、边界检查、Observation 的 window_id、
> 截图/标注/capture-protection** 六者必须指向同一个 window_id。

当前只有"都取焦点窗口"这一个巧合在维持它。

---

## 清点 3 — limit 语义

### 检索角度

```bash
grep -n -B1 "^        limit: usize,"      src/main.rs   # 角度 1：CLI 默认值
grep -n "_limit: usize"                   src/ax.rs     # 角度 2：被忽略的参数
grep -n "ax::snapshot(" src/*.rs | grep -v "limit"      # 角度 3：硬编码值
grep -n "ACTION_SNAPSHOT_ELEMENT_LIMIT"   src/main.rs   # 角度 4：响应侧独立上限
```

### A. CLI 默认值 —— 同一概念三个值

| 默认 | 命令（行号） |
|---|---|
| **50** | `Snapshot`(203) · `Perform`(294) · `SetValue`(330) · `Why`(822) · `State`(856) |
| **200** | `ObserveRegion`(401) · `Nearest`(433) · `Find`(476) · `Wait`(520) · `Click`(590) |
| 20 | `Commands`(164)（记录条数，与元素无关） |
| **无** | `Type` / `Key` —— **没有 `limit` 字段** |

> **r6 更正**：初稿把 294/330 错标成 `type`/`key`（实为 `Perform`/`SetValue`），
> 并把 `Wait`(520) 归进 200 组却未在正文提及。行号一直是对的，**命令名错了**。
> 这个错误直接传导进了计划 v4 的 C1，导致"把 set-value/perform 从 200 降到 50"
> 这种不存在的改动。**教训：枚举行号之后必须回读该行确认命令名，不能凭记忆映射。**

### B. 被完全忽略的 `limit` 参数（下划线前缀）

```
ax.rs:1378  pub fn ax_click(pid, ref_id, _limit)
ax.rs:1664  pub fn ax_find_element(pid, ref_id, _limit)
ax.rs:1672  pub fn ax_set_value(pid, ref_id, _limit, value)
ax.rs:1724  pub fn ax_perform(..., _limit, ...)
```

**ref 解析器全部无界。** `cu click --limit 200` 不约束解析，只约束前后校验快照。

### C. 命令内部硬编码

| 位置 | 值 | 场景 |
|---|---|---|
| `main.rs:2536` | 100 | `focused_input_before_type` |
| `main.rs:2645` | 50 | `cu type` 稳定性二次快照 |
| `main.rs:2955` | 50 | `cmd_key` 前置快照 |
| `main.rs:2773` `:2845` | `limit.max(50)` | `set-value` / `perform` 前置快照 |
| `main.rs:3676` `:3872` | 5 | `launch` / `warm` 预热 |

### D. 响应侧第三套上限

`main.rs:4587` `ACTION_SNAPSHOT_ELEMENT_LIMIT = 50` —— 动作响应里附带的快照
**无论 `--limit` 是多少都截到 50**（`main.rs:4607`）。

### 一次 `cu click 5 --app X` 涉及的元素基数

| 阶段 | 基数 | 来源 |
|---|---|---|
| 1. `pre_state` 快照 | **200** | click 的 `--limit` 默认 |
| 2. Observation 校验快照 | **50** | `expected.limit`，来自建 Observation 的 `cu snapshot`（默认 50） |
| 3. ref 解析 | **无界** | `_limit` 被忽略 |
| 4. 动作后快照 | **200** | click 的 `--limit` |
| 5. 响应内嵌快照 | **50** | `ACTION_SNAPSHOT_ELEMENT_LIMIT` |

> **一条命令，五个不同的元素基数。**
> 这把 r2-F4（"C1 直接复用 pre_state 会假性 stale"）从"两个快照 limit 不同"
> 升级为"`limit` 这个概念本身不自洽"。C1 不能只做局部适配。

### 必须成立的不变式

> 参与同一次身份判定的两份元素列表，必须由**相同的 limit 与相同的投影**产生。

---

## 清点 4 — axPath 生产与消费

### 检索角度

```bash
grep -n "pub ax_path\|rename = \"axPath\"" src/ax.rs                     # 角度 1：产出结构
grep -rn "\.ax_path\|ax_path\b" src/                                     # 角度 2：消费（全仓）
grep -n "build_path_segment\|compute_child_segment\|self_path" src/ax.rs # 角度 3：生成
grep -n "parse_path_segment\|child_matches_segment\|descend_to_ax_path" src/ax.rs  # 角度 4：解析
```

> **r3 更正**：初稿的角度 2 写成
> `grep ... src/main.rs src/broker.rs src/diff.rs src/wait.rs` —— **把 `src/ax.rs`
> 排除在外了**，因此漏掉了整套 axPath 解析器族和焦点传播（共 8 处）。
> 这正是 X0 存在的意义所在，却在 X0 自己身上重演了一次。角度 2 已改为全仓检索，
> 并新增角度 4 专门覆盖解析侧。消费者计数 **12 → 20**。

### A. 产出结构（2 处，**不止 `elements`**）

| 位置 | 结构 | 出现在 |
|---|---|---|
| `ax.rs:217` | `Element.ax_path` | `elements[]` |
| **`ax.rs:159`** | **`FocusedSummary.ax_path`** | **`focused` 对象，在 `elements` 之外** |

### B. 逻辑消费者（20 处，按子系统分组）

| 子系统 | 位置 | 依赖内容 | 剥离后的后果 |
|---|---|---|---|
| **Observation 身份** | `broker.rs:1762` `observed_generation` | 参与哈希 | **每次 ref action 假性 stale** |
| | `broker.rs:1779` `current_elements` | 拷贝进 `ObservedElement` | 同上 |
| | `broker.rs:1793` `same_element` | 逐位比对 | 同上 |
| **Electron/CEF 风险检测** | `main.rs:2444` `focused_inside_webarea` | 路径含 `webarea` | **`cu type` 的自动粘贴路由失效（R7 回归）** |
| | `main.rs:2787` `controlled_editor_risk` | 路径含 `webarea` | **`cu set-value` 的受控编辑器警告失效** |
| **焦点/输入验证** | `main.rs:2428` `same_focused_element` | 路径相等（含窗口标题变化的容错） | `cu type` 效果验证退化 |
| | `main.rs:2446` `focused_tree_value` | 路径匹配取 value | 同上 |
| | `main.rs:2459` `:2461` 同函数 | 同上 | 同上 |
| | **`main.rs:2498` `:2500` `enforce_text_input_focus`** | 路径相等 → **`focus_verified`** | **SKILL.md 要求 agent 先看 `focus_verified:true` 再输入 —— 这个字段会退化** |
| **click ax-path 模式** | `main.rs:3112` | 在 `pre_state` 里按 path 找 focus_target | 该模式的焦点校验失效 |
| **axPath 解析器族**<br>（r3 补入） | `ax.rs:1386` `parse_path_segment` | 反解 `Role[Title]:N` | — |
| | `ax.rs:1414` `child_matches_segment` | 逐段匹配子节点 | — |
| | `ax.rs:1460` `descend_to_ax_path` | 自顶向下下降 | — |
| | `ax.rs:1550` `with_ax_path` | 解析入口 | — |
| | `ax.rs:1601` `resolve_by_ax_path` | `cu click --ax-path` | — |
| | `ax.rs:1626` `ax_perform_by_path` | `cu perform --ax-path` | — |
| | `ax.rs:1641` `ax_set_value_by_path` | `cu set-value --ax-path` | — |
| **焦点传播**（r3 补入） | `ax.rs:1903` `detect_focused` | 把匹配到的 `Element.ax_path` 拷进 `FocusedSummary` | `focused.axPath` 恒为 null → 上面 6 个焦点/webarea 消费者全部退化 |

> 解析器族这一列的"剥离后果"是 `—`：它们消费的是**用户传入的 `--ax-path` 字符串**，
> 不是快照里的字段，所以 A2 的剥离不影响它们。
> 但它们引出一条 A2 之外、**B2 必须满足**的不变式（见下）。

### C. 生成点（3 处）

`ax.rs:222` `build_path_segment` · `ax.rs:879` `self_path` 拼接 · `ax.rs:991` `compute_child_segment`

### C2. 生成器与解析器必须互逆（r3 新增的不变式）

生成侧（C 组 3 处）与解析侧（B 组解析器族 7 处）是**两套独立实现的编解码器**，
没有共享代码，也没有往返测试：

```
生成: build_path_segment(role, title)  →  "role[title]"  →  self_path 用 "/" 拼接 + ":N"
解析: 按 "/" 切分 → parse_path_segment → child_matches_segment 逐段下降
```

> **不变式**：对任意元素，`descend_to_ax_path(snapshot 给出的 axPath)`
> 必须解析回**同一个真实 AX 元素**。

**这条不变式当前没有任何测试守护，且 B2 会直接威胁它**：
折叠产生的是**合成元素**（一个 ref 代表 row+cell+statictext 三个真实节点）。
必须明确定义：
1. 合成元素的 `axPath` 指向三者中的**哪一个**真实节点？
2. `cu click <ref>`（走 canonical iterator）与
   `cu click --ax-path <同一个 path>`（走 `descend_to_ax_path`）
   是否落到**同一个对象**？

若不定义，B2 之后这两条路径会静默分叉 —— 与清点 1 的 ref 分叉是同构的问题，
只是发生在 axPath 维度。

### D. 载荷占比（实测）

| 目标 | limit | 总字节 | axPath 字节 | 占比 | 单条平均 |
|---|---|---|---|---|---|
| Finder | 200 | 39,085 | 16,204 | **41%** | 81 |
| VS Code | 50 | 18,340 | 11,822 | **64%** | **236** |

### 必须成立的不变式

> 上表 B 组中**读取快照字段**的 12 个消费者（Observation 身份 3 + Electron 风险 2 +
> 焦点/输入验证 6 + click ax-path 模式 1）必须始终拿到**完整**的 `ax_path`；
> 解析器族 7 处消费用户传入的字符串，不受影响；`detect_focused` 是传播点，必须在剥离前运行。
> 剥离只能发生在序列化副本上，且必须在 `broker::publish_observation`（`broker.rs:832`）**之后**。

→ 这否定了 v2 的 `strip_ax_paths(&mut self)` 签名（r2-F10）：
`post` 快照在 `attach_captured_snapshot` 返回后仍被 `attach_verification` 和
`enforce_text_input_focus` 使用（`main.rs:3139-3147`），原地 mut 会打断它们。
正确签名是 `to_public_value(&self, with_ax_path: bool) -> serde_json::Value`。

---

## 对计划条目的影响

| 计划条目 | 清点结果如何改变它 |
|---|---|
| **X1** | 从"统一 2 份实现"→ **"建立 canonical iterator，5 个消费者全部改走它"**；必须同时覆盖批量读的**整体失败与部分失败**；等价性测试需要 action 路径回报自身 identity 的新出口 |
| **B2** | 折叠规则必须落在 canonical iterator 里；`project(role,size,pos)` 单节点谓词表达不了子树折叠，需要 traversal-level projector。**但 traversal-level projector 只解决表达能力，不修正规则本身**（r3）：装饰节点必须按 **actions/语义**识别，不能把"有 AXTitle"等同于"用户可操作"。另需定义合成元素的 axPath 归属（见 C2） |
| **B3** | 从"7 处 AX 站点"→ **"5 类机制、六个必须同窗的环节"**；机制 D 的启发式回退**无条件删除**（r6，含默认 Focused 路径）；默认独立定义为 `Focused`，**不把 `--window 1` == 默认写成契约**（r6） |
| **B4** | `window_bounds`（机制 A 第 7 行）必须跟随 `--window`，否则边界检查用错窗口 |
| **C1** | 不能只合并两次快照；需要先统一 limit 语义，否则 200 vs 50 必然假性 stale |
| **A2** | 消费者从 6 个修正为 **20 个**（其中 12 个读快照字段、7 个解析器族不受影响、1 个是传播点）；签名改为 `to_public_value`；`FocusedSummary` 与 `cu wait` 的嵌套快照都要覆盖；`detect_focused` 必须在剥离前运行 |
| **D1** | 单测清单新增：canonical iterator 全分支、批量部分失败注入、`--window` 六环节一致性 |

---

## 自查更正

**v2 计划把 `diff::id_of` / `content_changed` 列为 `ax_path` 消费者 —— 这是错的。**

```
$ grep -c "ax_path" src/diff.rs
0
```

`diff.rs:17` 的身份是 `(role, round(x), round(y))`，`:123` 的内容比对是
`title/value/width/height`，**都不涉及 `ax_path`**。
（`Element` 序列化进缓存文件时 `ax_path` 会随行，但无逻辑依赖它。）

→ A2 少一个约束：`--diff` 不受 axPath 剥离影响。上表 B 组已按核实结果修正为 12 个消费者，
不含 `diff.rs`。
