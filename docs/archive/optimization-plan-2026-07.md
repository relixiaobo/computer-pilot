# Computer Pilot — 优化与迭代计划 v8（2026-08-01）

> **Archived 2026-08-13.** This proposal was written against `main` at
> `ff5088b` (`v0.8.0`) and is retained as design history, not as an executable
> plan. Several assumptions and line references no longer match the current
> tree. The historical wording below describes its status at that time. For
> current priorities, use [`../ROADMAP.md`](../ROADMAP.md).

> **本文是唯一可执行规范。** v1/v2 的全部方案描述已被本文取代，不再有效。
> [`x0-inventory-2026-07.md`](./x0-inventory-2026-07.md) 是本文的**事实基础**（各概念的完整站点枚举），
> 其裁决已全部合入下方正文 —— **不要把两份文档当作"主计划 + 纠偏附录"共同解释**。
>
> **状态**: 经八轮 review（Codex），64 条 finding 全部并入。待九轮 review。
> **基线**: `main` @ `ff5088b` (v0.8.0)
> **基线验证**: `cargo clippy --all-targets -- -D warnings` 通过 · `cargo fmt --check` 通过 ·
> `bash tests/commands/run_all.sh` = **831 passed / 1 failed / 35 skipped / 867**
> （失败套件 `tell`，单独重跑 27/27 通过 → 见 D2）

## 修订记录

| 轮次 | 结果 |
|---|---|
| v1 | 初稿 |
| v2 | 并入 r1 的 10 条 |
| v3 | 并入 r2 的 10 条 + r3 的 6 条；新增 X0 清点；删除被否定的旧方案 |
| v4 | 并入 r4 的 7 条；C1 收敛为单一模型；X1 接口重做为统一 traversal |
| v5 | 并入 r5 的 7 条；基准门禁改为几何带约束；修正 C1 事实错误；B3 无条件删除启发式选窗 |
| v6 | 并入 r6 的 8 条；几何门禁补 null 拒绝；C1 撤回降默认；B4 覆盖右键/双击；B2 裁定 backing |
| v7 | 并入 r7 的 9 条；新增改动自检清单 |
| **v8** | 并入 r8 的 7 条；**基准 clean 判据补几何稳定性 + 多轮一致要求**；C1/A2/X1/C2c/B1a 的执行性缺口 |

### v8 相对 v7 的实质变更

| 条目 | v7（已作废） | v8 |
|---|---|---|
| 基准 clean 判据 | 只比剥离几何后的 identity；任一轮匹配即判等价 | **基线几何也须稳定该轮才算 clean**（宽带会吞掉真实回归：A₁.x=0 / A₂.x=1000 时 candidate x=500 被放行，实测）；**要求 3 个一致轮次**，一轮匹配不算证明 |
| B4 测试 | 所有越界场景→报错且不派发 | 与算法矛盾（算法说滚动恢复后应继续派发）。拆成**「可恢复→成功命中」与「恢复失败→不派发」两组**；补 ax-path 左键；用故障 seam 强制左键 AX 链失败 |
| C1 | 删除 `_limit` 参数 + 无条件用 enforcement 快照替代 pre-state | 二者都不可执行。**enforcement 返回 `{snapshot, limit}`；resolver 保留并使用该 limit；非 ref 模式（ax-path/text/coord）继续自取 pre-state** —— 否则 verify-by-default 对这三种模式直接消失 |
| A2 | "diff.rs 不引用 ax_path，不受影响" | **结论下错了**。`Diff.added/changed` 是 `Vec<Element>`，`main.rs:1917` 直接序列化 → `--diff` 默认仍泄露 axPath。需要**可复用的公共序列化器** |
| X1 故障注入 | PR 1 就测 `AXActionNames`/`AXSelected`/`AXFocused` | 这三个属性分别到 **PR 10 / PR 5** 才进生产路径，PR 1 测不了。X1 只交付**通用 fallback 机制**；各属性的注入归各自 PR |
| C2c | "传 pid + launchDate" | 若子进程仍靠 `running_apps_native()` 取 launchDate，**枚举一次没省** → 需按 PID 直查的 helper；且"见下方 C2c 专条"指向的专条**不存在** |
| B1a 协议 | bump 留到 B2 | B1a 改 `ObservedElement` 结构，而同版本常驻 Broker 会被复用 → **旧 Broker + 新 child = 所有 ref action 假性 stale**。**B1a 即 bump**，B2 再 bump 一次 |

### v7 相对 v6 的实质变更

| 条目 | v6（已作废） | v7 |
|---|---|---|
| X1 fallback 清单 | 不含 `AXSelected`/`AXFocused` | **补入**。B1a 把它们加进 batch 却没同步这张表 → 两侧同时读到 error marker 时 `None == None`，**B1a 这个安全门禁被自己重新开洞** |
| C1 边界测试 | `wait --ref 150`（不传 limit） | **必须 `--limit 50`**。`Wait` 默认本就是 200，v6 的测试改动前就能通过 —— 我刚更正完默认值表，转头写了条与它矛盾的测试 |
| C2c 正文 | 仍留着"杀掉→重启"旧测试 | 删除，指向纯函数 seam 专条 |
| A1 全位置剥离 | "无子命令同名所以安全" | **遇裸 `--` 必须停止解释** —— `cu type -- --client-key` 里那是用户文本 |
| A2 作用域 | "覆盖全部动作输出" | **动作快照维持永不含 axPath**；`--with-ax-path` 只限观测族；`SKILL.md:171` 同步 |
| A4 清理 | 结尾还原位置/尺寸 | **组合式 EXIT trap**：测试会自建 Finder 窗口（`test_window.sh:8`），且中断时结尾不执行 |
| B3 负向测试 | 只覆盖显式 `--window` | **补默认 Focused 路径** —— 那才是 `screenshot.rs:347` 回退实际发生的地方 |
| B4 测试 | 泛化 click 场景 | **四条 CGEvent 路径各一条**，并断言"目标 app 状态不变"而非只看退出码 |
| A3 归档 | "移入 archive，更新链接" | 列出**三个**具体失效链接 + CI 链接检查 |

---

## 改动自检清单

> **为什么需要它**：r5–r7 共 24 条 finding 中，**约一半是我修上一条时引入的新缺陷**
> —— 几何从"全严"矫枉到"全松"（r5-#1 → r6-#1）、C1 从"多选题"改成"错误的单选"（r6-#2）、
> B2 裁决又推回实现者（r6-#4）、B1a 加字段不同步 fallback 表（r7-#1）、
> 更正默认值表后写出与之矛盾的测试（r7-#2）。
>
> 这个比例说明问题不在具体条目，而在**我改文档时没有对改动本身做回归检查**。
> 靠"下次注意"解决不了，需要写成清单。

改动本计划的任何一条之前，逐项过：

1. **反例代入** —— 把本条提到的具体示例（Finder 四节点、`wait --ref 150`、
   某个 payload）**代进新规则算一遍**。三次出现"规则套不上自己举的例子"都是漏了这一步。
2. **测试能证伪吗** —— 新写的测试在**改动前必须失败**。写完问一句：
   "把这条改动删掉，这个测试还会绿吗？" 会绿就是白写。
3. **矫枉过正检查** —— 修的是"太严"还是"太松"？把钟摆推到另一端会放行什么？
   （几何门禁两轮都栽在这里）
4. **同源字段同步** —— 本条新增/删除了某个 AX 属性、输出字段、命令 flag 吗？
   全文搜一遍它出现的**所有**清单（fallback 表、故障注入表、消费者表、X0 枚举）。
5. **裁决而非转述** —— 出现"必须明确定义 / 二选一 / 由实现者决定"就是没完成。
   本文是唯一可执行规范。
6. **X0 同步** —— 本条推翻了 X0 的任何裁决吗？同一次改动里改掉。
7. **产出物自洽** —— 改动涉及 `bench-ab.py` 或 A5 补丁吗？跑它们自己的门禁。

### v6 相对 v5 的实质变更

| 条目 | v5（已作废） | v6 |
|---|---|---|
| 几何门禁 | 字段为 `None` 时 `continue` | **缺失/null/NaN/bool 一律判失败**。v5 的写法**实测可被"删掉全部几何字段"绕过** |
| C1 降默认 | `Wait`/`Find`/`Nearest`/`ObserveRegion` 200 → 50 | **撤回**。计划自己证明 Finder 前 50 个 ref 走不出侧边栏；且 `wait.rs:50` 只为 `--gone` 扩 limit，`snapshot --limit 200` → `wait --ref 150` 会**必然超时**而非"输出变小" |
| B4 边界预检 | 只锚定 `main.rs:3362`（左键 fallback） | **右键/双击分支（`main.rs:3352`、ax-path `main.rs:3117`）直接发 CGEvent，同样要过预检** |
| B2 backing | "必须明确定义"（未裁决） | **裁定单一 backing**，并规定 axPath/几何/selected/focused 各自取自哪个节点 |
| AXActionNames 三态 | 只在 B2 定义 | **补进 X1 故障注入清单**；B2 测试须分别覆盖 API error→Unknown、真空→折叠、非空→保留 |
| X1 `Projection::step` | 接收"已读入的子树" | 该签名会让 `limit` 失去约束 AX 工作量的作用；改为**惰性/有界 lookahead**，并对 visited-node 与 batch-read 次数加上界测试 |
| C2c PID 复用测试 | kill → 重启同 app | 几乎必得新 PID，**删掉 launchDate 校验也能通过**；改为纯函数/测试 seam 构造"同 PID 不同 launchDate" |
| X0 同步 | 滞后三处 | 已同步，并新增"计划推翻裁决必须同一次改动同步 X0"的规则 |

### v5 相对 v4 的实质变更

| 条目 | v4（已作废） | v5 |
|---|---|---|
| 基准几何门禁 | 几何仅作 drift 报告，不参与判定 | **几何必须落在 A₁/A₂ 基线带 ±2pt 内**；零尺寸塌缩直接拒绝。v4 的做法**实测会放行"按钮平移 1000px + 尺寸归零"的二进制** |
| B3 启发式回退 | 只在显式 `--window` 下禁用 | **无条件删除**。默认 Focused 路径在 AX 失败后同样会走 `screenshot.rs:347`，同样违反 `AGENTS.md:228` |
| C1 默认值清单 | 称 set-value/perform 为 200、type/key 有 limit | **事实更正**：set-value/perform **已经是 50**；type/key **根本没有 limit 字段**；**`Wait`（`main.rs:520`）是 200 且 v4 整条漏掉** |
| C1 limit 契约 | 解析器"真正实现 limit" + 命令 `--limit` 只管动作后快照 | 二者矛盾。**解析器改用 `expected.limit`（Observation 的），命令 `--limit` 只管动作后快照** |
| X1 故障注入 | 断言"五方 identity 一致" | 统一 traversal 后这是**同源自证**；改为**与无故障基线比对**，断言已知后代仍在、总数与后续编号不变 |
| B2 折叠阈值 | bbox 面积比 > 0.8 | 该阈值**折叠不了自己的示例**（statictext/cell = 0.451）；改为只对容器 wrapper 施加阈值，标签叶节点豁免 |
| B2 装饰判据 | `actions_supported` 为空 | `copy_action_names`（`ax.rs:1175`）**失败与真空都返回空 Vec**（源码注释自陈）；改为三态 `Known/KnownEmpty/Unknown`，仅 `KnownEmpty` 允许折叠 |
| B3 窗口序 | 断言 `--window 1` == 默认 focused | `AXWindows` 顺序无契约保证；**默认独立定义为 `Focused`**，索引严格对应 `cu window list`，不把二者绑成 API 契约 |

### v4 相对 v3 的实质变更

| 条目 | v3（已作废） | v4 |
|---|---|---|
| X1 接口 | `project(node, children) -> Option<Element>`，五个消费者各自保留遍历 | **单一 traversal**，产出 backing handle/path + descend/consume 决策；消费者只收集或在第 N 项回调 |
| X1 容错 | 只对 AXRole/AXPosition/AXSize 逐属性 fallback | **含 AXChildren**（漏了它会整棵子树消失）、label/value、B2 的 actions；逐属性故障注入 |
| B3 范围 | 观测 + 动作族 | **补 `wait` / `ocr` / `screenshot` / `click --text`**；`wait --new-window` 明确保持 app-scoped |
| C1 | 三处"或"的多选题 | **单一模型**：limit 统一为 50、`_limit` 真正实现、enforcement 权威取快照 |
| C2b | "独立线程 `child.wait()`" | 明确 **waiter 独占并 reap Child**；协调线程预存 PGID 按进程组发信号；补无残留进程测试 |
| A3 | "逐字节相同" | **跳过各自前言后比较**（`cmp -s` 实际返回 1） |
| A5 补丁 | 未过 fmt 门禁 | 已 `cargo fmt` 后重新生成 |

### v3 相对 v2 的实质变更

| 条目 | v2（已作废） | v3 |
|---|---|---|
| X1 | "统一 2 份实现" | **5 份实现全部改走 canonical iterator**；覆盖批量读整体+部分失败；测试读 action 路径自身 identity |
| B1b | `AXSelectedChildren` **计数** | **有序身份集合**；强制"基数不变但集合变化"回归测试 |
| B2 | 按"无 title"判装饰节点 | 按 **actions/语义**判定；实测证据见下；新增合成元素的 axPath 归属定义 |
| B3 | 五环节，只改 AX 侧 | **六环节含媒体捕获**；显式 `--window` 下**禁用**启发式回退 |
| B4 | 持有裸 `AXUIElementRef`；身份含几何 | **RAII 保留句柄/数组所有权链**；身份**剔除几何** |
| C1 | 直接复用 `pre_state` | 先统一 limit 语义（一次 click 现有 **5 个基数**），再谈合并 |
| A2 | `strip_ax_paths(&mut self)` | `to_public_value(&self, with_ax_path)`；消费者 6 → **20** |

---

## 目录

- [0. 测量方法](#0-测量方法)
- [X0 — 平行实现清点（前置）](#x0--平行实现清点前置)
- [X1 — canonical iterator + 现存 ref 错位 bug](#x1--canonical-iterator--现存-ref-错位-bug)
- [批次 A — 低风险速赢](#批次-a--低风险速赢)
- [批次 B — Agent 成功率](#批次-b--agent-成功率)
- [批次 C — 延迟](#批次-c--延迟)
- [批次 D — 工程基础](#批次-d--工程基础)
- [明确不做](#明确不做)
- [已裁决的开放问题](#已裁决的开放问题)
- [合入顺序](#合入顺序)

---

## 0. 测量方法

**工具**: `scripts/bench-ab.py`（已入仓、已验证）

四道闸：

1. **每样本校验** —— 退出码非 0、stdout 非 JSON、`ok != true` 一律 raise
   （**一个早早报错的二进制会显得更快**）
2. **等价门禁（身份严比 / 几何容差）** —— 见下
3. **交错采样 + 原始样本落盘** —— 每轮交替谁先跑，全部样本写进 `bench-ab-samples.json`
4. **Broker 隔离** —— `--via-broker` 时两个 arm 各自独立 `COMPUTER_PILOT_HOME`。
   必须如此：`ensure_running`（`broker.rs:581`）按 protocol+version 复用常驻 Broker，
   **同版本两个构建会共用同一个 Broker 进程**，而 C2 改的正是常驻 Broker ——
   否则两个 arm 跑的是同一份代码。

> **必须交错**：AX IPC 延迟取决于目标 app 当时在做什么。顺序跑在本机方差达 **3×**，
> 我首轮就得到过符号相反的假信号。**只跑单向顺序循环的复现，结论不可用。**

### 等价门禁的三次误判（工具自身的教训）

这个门禁是全部性能结论的依据，而它在使用中**连续给出三次错误判定**。
记录机制而不只是结论，因为同类错误很容易再犯。

| # | 错误设计 | 后果 | 修法 |
|---|---|---|---|
| 1 | 单次 A vs B 比对 | 基线对自己跑 3 次里 2 次 abort —— UI 漂移伪装成二进制差异 | 加控制组 |
| 2 | A-A-B 顺序 | 控制窗口**不覆盖** B 调用；B 期间的漂移被算到 candidate 头上。**对一份实际等价的补丁报了"real candidate regression"** | 改 A-B-A 包夹 |
| 3 | 单轮 A-B-A | 漂移值可能来回摆动使 A₁==A₂ 巧合成立，而 B 恰好采到偏移 | 多轮包夹，只采信"干净轮次"（A₁==A₂）；**任一干净轮次 B 也相等即判定等价** |

**第四个、也是最根本的问题**：全量逐字节比对**本身就是错的判据**。

决定性对照（同样包夹、同一目标 Finder `--limit 200`）：

```
A-vs-A (同一二进制)      clean=3/5   agree=3
A-vs-B (基线 vs 补丁)    clean=3/5   agree=0
```

看起来是铁证。但拆开字段后：

```
语义键 (ref/role/title/value/axPath)  200 个元素全同     ✓
几何键 (x/y/width/height)             仅 2/200 处不同
  idx 129  A (424,41,209,18)  B (424,41,220,18)
  idx 139  A (424,61,109,18)  B (424,61,209,18)
```

差异全在 Finder 文件列表的**列宽**上 —— 该区域在持续重排。
补丁把批量读提前了约 2–3 次 IPC，于是**在不同瞬间采到了动画中的宽度**。
语义没变，采样时刻变了。

→ 门禁改为：**身份键（ref/role/title/value/axPath）严格比对；
几何键约束在基线带内**。

> **v4 在这里矫枉过正（r5-#1）**：v4 把几何完全排出判定，只报 drift 计数。
> 实测构造一个把按钮 `x/y` 各平移 1000px、`width/height` 归零的 payload，
> **门禁判定 "identity identical" 予以放行**。
> 几何决定 CGEvent 点击坐标、`cu nearest`、`--region` 与标注截图偏移 —— 不是装饰字段。
>
> v5 的规则：A₁ 与 A₂ 在时间上包夹 candidate，`[min(A₁,A₂), max(A₁,A₂)]`
> 就是 UI 在该窗口内合法占据的范围。candidate 的每个几何字段必须落在该带 **±2pt**
> 内；此外**基线非零而 candidate 为 0 的宽高一律拒绝**（塌缩不是漂移）。

```
equivalence: identity identical (27508 bytes) in a baseline-bracketed round
             (1 clean of 5); 3 element(s) differ in geometry only
             -- live relayout, not a semantic change
```

### 第五个问题：`clean` 指标本身不检查几何（r8-#6）

v7 的 `clean` 只比较**剥离几何后**的 identity。于是：

- 一个基线几何剧烈漂移的轮次**照样算 clean**
- 而 `geometry_verdict` 用 `[min(A₁,A₂), max(A₁,A₂)]` 作带 —— **基线漂得越狠，带越宽**

实测：`A₁.x=0`、`A₂.x=1000` 时，candidate `x=500` **被判通过**。
所以 v7 声称的"`clean=N/5` 即 quiescent 指标"是无效的，宽带能吞掉真实坐标回归。

v8 的规则：

1. **一轮算 clean，要求基线的 identity 与几何都稳定**
   （`geometry_stable`，A₁↔A₂ 每个字段差 ≤ 4pt）
2. **要求 3 个一致轮次**（`REQUIRED_AGREEING`），最多试 8 轮 ——
   "任一轮匹配即等价"在噪声目标上撞运气的概率太高
3. 干净轮次不足 3 个 → 退出码 3（目标太吵），**不是**指控 candidate

### 目标可用性

**并非所有目标都能当基准。** 本轮 Finder 处于活跃重排状态：
`--limit 120` 基线中位数 **582.9 ms**，而同一会话稍早 `--limit 200` 只要 ~130 ms。
这种目标上的时延数字不可采信（同一配置连续三次测得 −52% / −57% / −64%）。

**规则**：报告性能结论前，先确认目标 quiescent（门禁的 `clean=N/5` 即该指标，
以及基线中位数与历史量级是否一致）。VS Code 在本机跨会话稳定，是更可靠的基准目标。

**待补**：门禁的三条 abort 路径（不等价 / 目标不静止 / 二进制报错）
应在 D1 补自动化测试；同二进制作为两个 arm 时 `cand_samples` 为空会抛异常，也需修。

---

## X0 — 平行实现清点（前置）

**优先级**: P0 · **状态**: 已完成，见 [`x0-inventory-2026-07.md`](./x0-inventory-2026-07.md)

四个概念的完整枚举（每项用 3–5 个独立检索角度交叉验证）：

| 概念 | 基数 | 关键发现 |
|---|---|---|
| ref 编号 | **5 份实现** | `ax.rs:936/1217/1264/1308/1805`，四角度收敛 |
| 窗口解析 | **5 类机制** | 含 v2 漏掉的媒体捕获路径与 CGWindowList 启发式回退 |
| limit 语义 | **一次 click 涉及 5 个基数** | `_limit` 参数被 4 个解析器完全忽略 |
| axPath | 2 产出 + **20 消费** + 3 生成 | 含解析器族 7 处、传播点 1 处 |

**X0 是 X1 起所有条目的前提。** 每条改动的目标清单直接引用 X0 的枚举表。

---

## X1 — canonical iterator + 现存 ref 错位 bug

> **这不只是重构，其中含一个当前版本已存在的静默错元素 bug。**

**优先级**: P0 · **工作量**: 2–3 天 · **风险**: 中（收敛性改动，无行为放松）

### 问题

X0 清点 1 确认 ref 编号有 **5 份独立实现**，靠"谓词恰好相同"维持一致：

| # | 函数 | 位置 | 服务命令 | 属性读取 |
|---|---|---|---|---|
| 1 | `walk` | `ax.rs:936` | snapshot 及全部前后快照 | **批量** |
| 2 | `find_and_perform_action` | `ax.rs:1217` | `cu perform` | 单属性 |
| 3 | `find_and_set_value` | `ax.rs:1264` | `cu set-value` | 单属性 |
| 4 | `find_element_by_ref` | `ax.rs:1308` | `cu click` | 单属性 |
| 5 | `find_and_inspect` | `ax.rs:1805` | `cu why` | 单属性 |

**已经不一致的两条路径**：

**(a) 批量读整体失败** —— `walk`（`ax.rs:888`）在 `values.is_null()` 时
只递归子节点、**不 emit 自己、不 +1**；其余四份不走批量，正常 +1。

**(b) 批量读部分失败** —— `AXUIElementCopyMultipleAttributeValues(..., options=0)`
可返回**非空数组但个别项是 error marker**。`batch_string`（`ax.rs:783-797`）
已明确在检查 `CFGetTypeID != CFStringGetTypeID` 并返回 `None`
—— **说明这条路径真实存在**。此时 `AXRole` 取不到 → `walk` 不 +1，
而 `values.is_null()` 的 fallback **不会触发**；其余四份用单属性读能拿到 → +1。

两种情况都导致：**该节点之后的所有 ref 错位一格。**

Observation 门禁挡不住 —— `same_element` 比对的两侧分别来自实现 #1 和实现 #4，
用不同算法，比对前提本身不成立。

### 改动

**1. 单一 traversal，不是"共享谓词 + 五份遍历"**

v3 提的 `project(node, children) -> Option<Element>` 表达不了 B2 需要的东西（r4-#1）：
在 `row → cell → statictext` 里，**`row` 看不到孙节点，`cell` 看不到父节点**；
返回值也无法表示"消费整条子树"或"实际要操作的是哪个 AX 节点"。
保留五份遍历就必然保留五处分叉点。

真正的统一遍历 —— **遍历只有一份，消费者只负责收集或在第 N 项回调**：

```rust
// src/ax.rs
/// 一次遍历产出的一个 ref。`backing` 是实际要操作的 AX 节点
/// （折叠时可能不是最外层，也不是最内层 —— 由投影策略指定）。
struct Projected {
    element: Element,          // 对外的 ref/role/title/value/geometry/axPath
    backing: BackingHandle,    // 保留所有权的 AX 句柄，见 B4
    consumed_subtree: bool,    // 该 ref 是否吃掉了整条子树
}

enum Step { Skip, Descend, Emit(Projected), EmitAndConsume(Projected) }

/// 投影策略。看得到当前节点、其祖先链，以及**按需惰性展开**的后代。
///
/// `Lookahead` 不是"已读入的子树" —— 那样为了决定根节点的投影就得先读整棵树，
/// `limit` 会彻底失去约束 AX 工作量的作用（r6-#6）。
/// 它按需拉取，且受 `MAX_COLLAPSE_LOOKAHEAD` 限制（B2 的单链折叠最多向下看 3 层）。
trait Projection {
    fn step(&mut self, node: &NodeView, ancestors: &[NodeView], look: &mut Lookahead) -> Step;
}

/// 有界惰性前瞻。`child(i)` 触发一次 batch read；超过上限返回 None，
/// 调用方必须能在信息不全时做出保守决策（保留 ref，不折叠）。
struct Lookahead { /* ... */ }
const MAX_COLLAPSE_LOOKAHEAD: usize = 3;

/// 唯一的遍历实现。消费者传一个回调，返回 ControlFlow 决定继续还是停。
fn traverse<F>(root: CFTypeRef, projection: &mut dyn Projection, limit: Option<usize>, f: F)
where F: FnMut(usize, &Projected) -> std::ops::ControlFlow<()>;
```

**五个消费者退化为回调**：
- `snapshot`：收集全部，`limit` 到即 `Break`
- `click` / `set-value` / `perform` / `why`：在第 N 项 `Break` 并取 `backing`

**2. 逐属性 fallback 必须覆盖所有影响遍历或投影的属性**（r4-#2）

v3 只列了 `AXRole`/`AXPosition`/`AXSize`，**漏掉最严重的 `AXChildren`**：

`batch_children`（`ax.rs:836`）在 error marker 上返回 `None`，
而 `walk`（`ax.rs:959`）是 `if let Some(children) = batch_children(...)` ——
**拿不到就完全不递归，整棵子树从 snapshot 消失**；
四个 action walker 用 `ax_attr(element,"AXChildren")`（`ax.rs:1226/1271/1323/1830`）
照常递归。ref 不是错位一格，是**错位整棵子树的元素数**。

需要逐属性 fallback 的完整清单：

| 属性 | 影响 | 失败后果 |
|---|---|---|
| **`AXChildren`** | 遍历 | **整棵子树消失** |
| `AXRole` | 投影（是否发 ref） | 错位一格 |
| `AXPosition` / `AXSize` | 投影（尺寸门槛）+ 几何 | 错位一格 |
| `AXTitle` / `AXDescription` / `AXHelp` / `AXIdentifier` | 标签链 + axPath segment | axPath 分叉 |
| `AXValue` | 标签 | 身份比对失败 |
| `AXActionNames`（B2 新增） | 装饰节点判定 | 折叠决策分叉 |
| **`AXSelected` / `AXFocused`（B1a 新增）** | **Observation 身份** | **两侧同时读到 error marker → `None == None` → 选中态变化被判为"未变"，B1a 要防的"删错文件"场景重新放行** |

> **最后一行是 v6 的自伤**（r7-#1）：B1a 把这两个属性加进 `BATCH_ATTR_NAMES`，
> 却没有同步加进本表。它们服务的是**安全门禁**，退化后果比其它字段更重 ——
> 不是少一个 ref，是让一次错误的破坏性操作通过校验。
>
> 额外要求：**区分 "unsupported"（该元素本就没有此属性）与 "读取失败"**。
> 前者是合法的 `None`，后者必须走 fallback；两者混为一谈时，
> 一个不暴露 `AXSelected` 的元素会掩盖一次真正的读取失败。

**3. 新增 action 路径的 identity 回报出口**（测试用）

### 测试

**等价性测试（核心）**

r2-F1 指出 v2 的测试无效：`cu why` 的 `el` 来自它刚跑的 snapshot（实现 #1），
而 `inspect_ref`（实现 #5）是另一次遍历，**身份从不与 `el` 比对**（`main.rs:3766-3776`）。
拿 `cu why` 的 axPath 比 snapshot 的 axPath = snapshot 比 snapshot，测不到分叉。

→ 新增测试专用输出（env gate，参考 `system.rs` 的 `CU_TEST_FRONTMOST_OVERRIDE` seam）：
每个 ref 消费者在解析成功后回报它**自己**解析到的
`(ref_id, role, title, axPath, x, y, w, h)`。
断言：**五个消费者对同一 `ref_id` 回报的 identity 完全一致。**

**逐属性故障注入**（上表每一行各一条）

> **注意：统一 traversal 之后，"五方 identity 一致"变成了同源自证**（r5-#4）。
> 五个消费者共用一份遍历，若 `AXChildren` 的 fallback 仍有 bug，
> 它们会**一致地**丢掉整棵子树 —— 断言依然全绿，而 bug 原封不动。
>
> 故障注入的断言对象必须是**无故障基线**，不是彼此。

每个注入场景断言三件事（与同一目标的无故障快照比对）：

1. **已知后代仍然存在** —— 注入前先记录该节点子树中某个具名叶子（如 Finder 侧边栏
   某个条目的 statictext），注入后它必须仍在结果里
2. **元素总数不变**
3. **该节点之后的 ref 编号不变**（错位的直接判据）

**注入范围必须与 PR 顺序一致**（r8-#4）：

> v7 让 X1（PR 1）测 `AXActionNames`、`AXSelected`、`AXFocused` ——
> 但这三个属性分别到 **B2（PR 10）** 和 **B1a（PR 5）** 才进入生产路径。
> PR 1 时它们还不在 `BATCH_ATTR_NAMES` 里，**测不了**。

| 属性 | 注入测试归属 |
|---|---|
| `AXChildren` · `AXRole` · `AXPosition` · `AXSize` · 标签链 · `AXValue` · 整体 null | **X1（PR 1）** —— 共 **7 类** |
| `AXSelected` / `AXFocused` | **B1a（PR 5）** —— 随字段入生产同批交付 |
| `AXActionNames` | **B2（PR 10）** —— 同上 |

X1 只交付**通用的逐属性 fallback 机制 + 上述 7 类注入**；
后两批在各自 PR 的 gate 里补齐，**并同步更新本节的验收数量**。

> 最后一条是 v5 漏掉的。B2 用 actions 判定装饰节点，而
> `copy_action_names`（`ax.rs:1175`）失败与真空都返回空 `Vec`。
> 三态设计（`Known`/`KnownEmpty`/`Unknown`）写在 B2 里，但**没有测试守护 FFI adapter
> 是否真的把 error 映射成 `Unknown`** —— `Projection::step` 的单测发现不了这一层。
>
> 注入断言：`AXUIElementCopyActionNames` 返回非 `AX_OK` 时，
> 该 ref **必须保留**（不得被当作装饰节点折叠掉）。

**外加一条元测试**：把 `AXChildren` 的 fallback 故意注释掉，
上述测试**必须失败**。不能失败的测试不算守护。

**单测**：`Projection::step` 全分支（D1 覆盖）

### 验收

- 五个消费者的 identity 等价性测试在 Finder / VS Code / TextEdit 上通过
- 上表 **7 类**故障注入下 ref 均不错位（`AXSelected`/`AXFocused` 归 B1a、
  `AXActionNames` 归 B2，见上方归属表）
- 831 基线不回退

---

## 批次 A — 低风险速赢

### A1. 全局 flag 加 `global = true`（选择性）

**优先级**: P0 · **工作量**: 30 分钟 · **风险**: 低

**问题**

```
$ cu snapshot Finder --json
{"code":"invalid_argument","error":"error: unexpected argument '--json' found\n\n
  tip: to pass '--json' as a value, use '-- --json'\n"}
```

全局 flag 必须写在子命令之前，而 LLM 的自然写法是写在后面，clap 的 tip 还会把它引向更错的命令。

**改动**

只把四个设为 `global = true`：`human`(`main.rs:133`) · `json`(`:137`) ·
`client_key`(`:141`) · `request_id`(`:145`)。

**`--timeout`（`main.rs:149`）不动** —— root 单位是**毫秒**，
`cu wait`/`cu launch` 的子命令级 `--timeout` 单位是**秒**。全局化会撞名，
且两个单位共用一个 flag 名对 Agent 是更大的坑。

**`broker_child_argv` 按 flag 分别处理**（`main.rs:1315`）：

```
--client-key / --request-id  →  全位置剥离，但遇到裸 `--` 即停止解释
--timeout                    →  保持 root-scoped 剥离（子命令同名且语义不同）
```

> **"无子命令同名所以安全"是错的**（r7-#4）。裸 `--` 之后的一切都是**用户文本**：
>
> ```bash
> cu type -- --client-key      # 用户要输入的字面量，不是全局参数
> ```
>
> 全位置扫描会把它当 flag 吃掉，用户少打两个词。
> 扫描器必须在遇到裸 `--` 时停止解释，其后原样传递。

**回归测试**：`cu type -- --client-key` 与 `cu type -- --request-id`
经 Broker 后，目标应收到字面文本；断言 `broker_child_argv` 未剥离它们（D1 单测）。

> v2 曾写"改成扫描全部位置" —— **那是错的**：对 `--timeout` 全位置扫描会把
> `cu wait --timeout 10` 的值一起吃掉。

**测试**
- `cu snapshot Finder --json` 退出 0 且可解析；`cu --json snapshot Finder` 行为不变
- `cu snapshot --json Finder --client-key k` 与 `cu --json --client-key k snapshot Finder` 的 `client_key` 一致
- **Broker 路径回归**：`cu wait --timeout 3` / `cu launch <app> --timeout 3` /
  `cu tell <app> '...' --timeout 3` 三条经 Broker 正常执行，子命令 timeout 不被剥离
- `broker_child_argv` 提为纯函数并单测（D1）

---

### A2. `axPath` 默认不输出

**优先级**: P0 · **工作量**: 1 天 · **风险**: 中高

**收益**（实测）

| 目标 | limit | 总字节 | axPath | 占比 | 单条平均 |
|---|---|---|---|---|---|
| Finder | 200 | 39,085 | 16,204 | **41%** | 81 |
| VS Code | 50 | 18,340 | 11,822 | **64%** | **236** |

`cu state`（SKILL.md 的规范首调用）Finder 一次 11,559 字节 ≈ 2,900 tokens。

**改动 —— 序列化副本，不原地修改**

v2 提的 `strip_ax_paths(&mut self)` 是错的（r2-F10）：`post` 快照在
`attach_captured_snapshot` 返回后仍被 `attach_verification` 与
`enforce_text_input_focus` 使用（`main.rs:3139-3147`），原地 mut 会打断它们。

```rust
impl SnapshotResult {
    /// 产出面向 CLI 的序列化副本。原对象不变 —— 内部消费者始终看到完整 ax_path。
    fn to_public_value(&self, with_ax_path: bool) -> serde_json::Value
}
```

**必须同时剥离两处产出**（X0 清点 4 A 组）：
`Element.ax_path`（`ax.rs:217`）**和** `FocusedSummary.ax_path`（`ax.rs:159`，在 `elements` 之外）。

**必须覆盖的输出面**：`cmd_snapshot` / `cmd_state` / `cmd_find` / `cmd_nearest` /
`cmd_observe_region` / **`cmd_wait`（`main.rs:2331` 返回完整嵌套 snapshot）** /
全部动作命令的 `attach_captured_snapshot`。

**动作快照是一条独立契约，必须单独裁决**（r7-#5）：

`compact_action_snapshot`（`main.rs:4611`）**无条件** `element_obj.remove("axPath")`，
而 `SKILL.md:171` 把它写成了公开契约：
*"Action snapshots omit `axPath`, cap text, and include at most 50 elements."*

→ 裁定：**动作快照维持"永不含 axPath"，`--with-ax-path` 不作用于它。**
理由：动作快照的定位是"下一步够不够用"的低成本回执，
axPath 在其中的价值最低而占比最高（VS Code 上 64%）。

因此 `--with-ax-path` 的作用域**只限观测族**：
`snapshot` / `state` / `find` / `nearest` / `observe-region` / `wait`。
动作命令**不提供**该 flag —— 提供一个不起作用的 flag 比没有更糟。

**文档同步**：`SKILL.md:171` 与 `references/commands.md` 必须写明
"`--with-ax-path` 仅观测族可用；动作快照始终不含 axPath"，
并更新观测族输出的默认说明（现在默认也不含了）。

**顺序约束**：
- `broker::publish_observation`（`broker.rs:832`）必须在剥离**之前**
- `detect_focused`（`ax.rs:1903`，把 `Element.ax_path` 拷进 `FocusedSummary`）必须在剥离**之前**

**必须保持完整的 12 个快照字段消费者**（X0 清点 4 B 组）：

| 子系统 | 位置 | 剥离后的后果 |
|---|---|---|
| Observation 身份 | `broker.rs:1762` `:1779` `:1793` | **每次 ref action 假性 stale** |
| Electron 风险检测 | `main.rs:2444` `focused_inside_webarea` | **`cu type` 自动粘贴路由失效（R7 回归）** |
| | `main.rs:2787` `controlled_editor_risk` | `cu set-value` 受控编辑器警告失效 |
| 焦点/输入验证 | `main.rs:2428` `:2446` `:2459` `:2461` | `cu type` 效果验证退化 |
| | `main.rs:2498` `:2500` | **`focus_verified` 退化 —— SKILL.md 要求 agent 先看它再输入** |
| click ax-path 模式 | `main.rs:3112` | 该模式焦点校验失效 |

解析器族 7 处（`ax.rs:1386`–`1641`）消费的是用户传入的字符串，**不受影响**。

**`--diff` 分支必须单独处理**（r8-#3）：

> v7 写"`diff.rs` 零引用 ax_path，不受影响" —— **从"算法不依赖它"错误推出了"输出不含它"**。
>
> `Diff.added` / `Diff.changed` 的类型是 **`Vec<Element>`**（`diff.rs:78,80`），
> `main.rs:1916` 起把整个 `Diff` 直接序列化进响应；`focused` 在 diff 分支也是单独序列化的。
> → **`cu snapshot --diff` 默认仍会泄露 axPath**，A2 的收益在这条路径上归零。

因此 A2 不能只在 `SnapshotResult` 上加方法，必须建立**可复用的公共序列化器**：

```rust
fn element_to_public(e: &Element, with_ax_path: bool) -> Value
fn focused_to_public(f: &FocusedSummary, with_ax_path: bool) -> Value
fn diff_to_public(d: &Diff, with_ax_path: bool) -> Value   // 复用上面两个
```

**测试**：`--diff` 首次调用（返回全量）、后续调用（返回增量）、`--with-ax-path`
三种情况都断言 axPath 的存在/缺席符合预期。

**`cu why` 列为显式例外**：它是诊断命令，`--ax-path` 是它的入参也是回显，
始终输出路径，不受 A2 影响 —— 在 `references/commands.md` 写明。

**schema 常量两处同步**：`main.rs:24` `MACHINE_SCHEMA_VERSION` 与
`compatibility.json:8`，`1.0` → `1.1`；README 示例同步。

**测试**
- 36 处现有断言改为：不带 flag 断言缺席，带 `--with-ax-path` 断言存在
- 行为测试：两种模式除 `axPath` 外逐字段相同
- **回归（守护上表每一行）**：
  - `cu snapshot`（无 axPath）→ `cu click <ref>` 成功，不 `stale_observation`
  - **`cu type` 对 Electron/CEF 输入仍返回含 `AXWebArea` 的 `paste_reason`**
  - **`cu click` 对文本输入仍返回 `focus_verified:true`**
  - `cu wait` 返回的嵌套 snapshot 同样不含 axPath

---

### A3. 修正 README / 归档过期文档 / AGENTS.md 去重

**优先级**: P1 · **工作量**: 30 分钟 · **风险**: 零

| 位置 | 声称 | 实测 |
|---|---|---|
| `README.md:22` | 1.3MB | **2.56 MB** |
| `README.md:23` | <10ms | **~100–205 ms** |
| [`docs/competitive-analysis.md:48,50`](../competitive-analysis.md) | 1.2 MB / <10 ms | 同上 |

- 延迟改为 `~100 ms（AX 快照，Finder limit=50 中位数）` + 脚注指向 `scripts/bench-ab.py`；
  竞品数字标注"厂商声称，未复现"
- [`docs/competitive-analysis.md`](../competitive-analysis.md) 自称 `frozen at 2026-04-03` 却仍被 ROADMAP 链接
  → 移入 `docs/archive/`。**迁移会产生三个失效链接**（r7-#9），必须一并处理：

  | 位置 | 现状 | 处理 |
  |---|---|---|
  | `docs/README.md:45` | `[competitive-analysis.md](../competitive-analysis.md)` | 改指 `archive/competitive-analysis.md` |
  | `docs/ROADMAP.md:7` | `` [`competitive-analysis.md`](../competitive-analysis.md) `` | 同上 |
  | `docs/competitive-analysis.md:3` | `./ROADMAP.md`（文件自身的出链） | 目录下移一层 → `../ROADMAP.md` |

  **并把本地链接检查加进 CI** —— 这类迁移每次都会漏一两处，人工核对不可靠
- `AGENTS.md` 与 `CLAUDE.md` **不是**逐字节相同（r4-#6）：

  ```
  $ cmp -s AGENTS.md CLAUDE.md ; echo $?
  1                                    # AGENTS.md 多 5 行 Codex 说明前言
  $ diff <(tail -n +6 AGENTS.md) <(tail -n +2 CLAUDE.md) ; echo $?
  0                                    # 跳过各自前言后正文相同
  ```

  → CI 断言必须**跳过各自前言后比较正文**（或改为从一份 canonical body 生成两个文件）。
  照"逐字节相同"实现会永久失败。
  **不用 symlink**（部分 checkout/打包流程会退化成文本文件）。

---

### A4. `test_window.sh` 还原用户窗口

**优先级**: P1 · **工作量**: 15 分钟 · **风险**: 零

`test_window.sh:65` 把 Finder 移到 (250,150)、`:86` resize 成 900×600，**结束不还原**。
对照 `test_stale_state.sh:22` 的规范做法。对一个以"不打扰用户"为核心卖点的项目，应当修。

**改动**（r7-#6 补强）：v6 只说"结尾还原"，仍不完整 ——

1. `test_window.sh:8-10` 在没有 Finder 窗口时**用 osascript 新建一个**：

   ```bash
   osascript -e 'tell application "Finder"
     if (count of Finder windows) is 0 then make new Finder window
   end tell'
   ```

   还原位置/尺寸救不了这种情况 —— 用户桌面上会**多出一个窗口**。

2. **结尾还原在中断时根本不执行**（Ctrl-C、超时、断言 `exit 1`）。

正确做法：注册**组合式 `EXIT` trap**（保留 helpers 现有的 `cleanup_run`，不要覆盖）：

- 开头记录：是否存在 Finder 窗口、若存在则其 position + size
- trap 内：**原本就有窗口** → 还原 position + size；
  **原本没有** → 关闭本测试创建的那一个（按记录的窗口 id，不是"关闭前窗口"）

**验收**：①连跑两次 `run_all.sh window`，`window_frame` 前后一致；
②测试中途 `kill -INT`，Finder 窗口数与位置仍恢复原状。

---

### A5. 合入 AX walker 的 batch 复用补丁

**优先级**: P1 · **工作量**: 已完成 · **风险**: 低

**补丁**: [`a5-ax-walker-batch-reuse.patch`](./a5-ax-walker-batch-reuse.patch)
（历史候选补丁；归档时仍可应用，但尚未合入）

`compute_child_segment`（`ax.rs:991`）在递归前对每个子节点单独读
`AXRole`/`AXTitle`/`AXDescription`，而 `BATCH_ATTR_NAMES`（`ax.rs:720`）已含这三个。
改为父节点做一次 `batch_read`，同时供 segment 计算和递归使用。

**内存安全**：补丁把 `values` 所有权移入 `walk`，而 `walk` 有 4 个早退分支
（limit / MAX_DEPTH / null / 正常）。用 RAII guard（`BatchValues` + `Drop`）覆盖全部出口
—— 首版补丁在 limit 与 MAX_DEPTH 两处泄漏（r2-F8），已修。

**门禁自洽**：补丁须自身通过本计划的 gate。首版 `git apply --check` 通过但
隔离应用后 `cargo fmt --check` 失败两处（r4-#7），已 `cargo fmt` 后重新生成。
**验证方式**：干净副本 → `git apply` → `cargo fmt --check` + `cargo clippy -D warnings`
全过。这条应纳入 CI（见 D1）。

**等价性**：身份键（ref/role/title/value/axPath）在 Finder `--limit 50/120/200`
与 Code `--limit 50` 上**逐元素全同**；`--limit 120/200` 有 2–8 处几何键差异，
已查明为 Finder 列宽实时重排（详见 §0）。

**实测**

| 场景 | 基线 | 优化后 | 改善 | 可信度 |
|---|---|---|---|---|
| **Code limit=50** | 109.4 / 86.9 ms | 99.0 / 78.4 ms | **−9.5% / −9.7%** | **高**（跨会话两次一致，目标静止） |
| Code limit=200 | 167.2 ms | 147.0 ms | −12.1% | 中 |
| Finder limit=50 | 99.5 / 75.3 ms | 92.9 / 70.0 ms | −6.6% / −7.1% | 中 |
| Finder limit=200 | 134.8 / 140.1 ms | 124.5 / 132.3 ms | −7.7% / −5.5% | 中 |
| ~~Finder limit=50/120（末轮）~~ | ~~207.5 / 582.9 ms~~ | ~~206.5 / 236.3 ms~~ | ~~−0.5% / −59.5%~~ | **作废** —— Finder 当时活跃重排，基线量级偏离历史 4×，见 §0「目标可用性」 |

**结论取 −9.5%（Code，静止目标，跨会话复现）**。作废行保留在表内，
是为了让 reviewer 看到"哪些数字被丢弃、为什么"，而不是只看到留下的那些。

> **诚实说明**：原预期 2–3×，实测约 **−9.5%**。瓶颈不在 IPC 次数，而在进程启动、
> dyld 加载 Foundation/Vision/SCK、NSWorkspace 枚举。**这是"免费的 ~10%"，不是银弹。**

**顺序**：X1 会重写 `walk` 的批量失败分支 —— **X1 先合，A5 在其上 rebase**。

**测试**：等价性测试放本 PR，用 `scripts/bench-ab.py` 的等价门禁 + 一条 shell 断言，
**不依赖 A2 的 `--with-ax-path`**。

---

## 批次 B — Agent 成功率

### B1. 补齐 Observation 的语义状态，再收窄 drift 模型

**优先级**: P0 · **工作量**: 2–3 天 · **风险**: 中

### 两个模型都是错的

**现状过宽**：`observed_generation`（`broker.rs:1751`）哈希全树 9 个字段 ——
一个时钟、进度条、spinner 就让完全没动过的按钮拒绝点击。

**但 `same_element` 也不够**（r1-F4 反例成立）：
Finder 选中文件 A → snapshot 拿到工具栏 Delete 按钮 ref 12 → 选中变成 B →
`same_element(ref 12)` 依然通过（按钮的 role/title/value/几何/path 一个字节没变）→ **删错文件**。

**而现状对这个反例也不设防**：`observed_generation` 只哈希
`ref_id·role·title·value·x·y·w·h·ax_path`，`BATCH_ATTR_NAMES`（`ax.rs:720`）
只读 9 个属性 —— **`AXSelected`/`AXFocused` 从头到尾没被读进 snapshot**。
（`AXSelected` 只在**写**的一侧出现：`ax.rs:1095` 点击链第 8 步。）
若某 app 的选中态只体现为高亮渲染，全树哈希同样放行。

### 改动（三步，可分 PR）

**B1a 必须自带协议 bump**（r8-#7）

> B1a 改的是 `ObservedElement` 的**结构**（加 `selected`/`focused`）与
> `observed_generation` 的**算法**。而 `ensure_running`（`broker.rs:581`）
> 按 protocol + version 复用常驻 Broker ——
> **旧 Broker 持有的 Observation 没有这两个字段，新 child 用新算法算 generation**
> → 典型结果是**所有 ref action 假性 stale**。
>
> 更糟的是：B1a 的负向测试（选中漂移 → 必须 stale）会因此**错误地通过** ——
> 它本来就期望 stale，而假性 stale 也是 stale。那条测试证明不了洞被补上。

→ **B1a 即 bump `INTERNAL_PROTOCOL`（`broker.rs:23`，2 → 3）**；
B2 的 ref 重编号再 bump 一次（3 → 4）。

→ **B1a 必须同时有一条正向测试**：UI **完全没变**时 ref action **成功**。
没有它，"假性 stale" 与"正确 stale" 无法区分。

**B1a — 补洞（只增严，不放松，可独立上线）**
1. `BATCH_ATTR_NAMES`（`ax.rs:720`）加 `AXSelected` / `AXFocused`
2. `Element` / `ObservedElement` 加 `selected: Option<bool>` / `focused: Option<bool>`
3. `observed_generation` / `same_element` 纳入这两个字段

**B1b — 收窄模型**（需 B1a 覆盖为前提）

drift 判定从"全树哈希相等"改为三项**全部**相等：
1. 目标元素完全相等（`same_element` + B1a 的 selected/focused）
2. 目标元素的**祖先链**相等（用 `ax_path` 前缀，已有数据，零额外 IPC）
3. **窗口级选中集合的有序身份集合**相等

> **第 3 项必须是身份集合，不能是计数**（r2-F3 / r3-#2）。
> v2 写的 `AXSelectedChildren` **计数** 会重新放行上面的删错文件反例 ——
> A、B 各单选一项时计数恒为 1，若焦点仍在工具栏，目标、祖先、focused path 也都不变，
> B1a 加入的逐元素 selected 状态在收窄后不再参与判定。
>
> 正确做法：记录 `AXSelectedChildren` 中每个元素的 **axPath（有序）**，整体比对。

**B1c — 无关子树漂移放行**：只有 B1b 有充分测试覆盖后才做。

### 测试

- **保留现有行为**：`test_stale_state.sh` 的"移动窗口 → stale"必须仍失败
- **B1a 核心证据测试**：Finder 选中文件 A → snapshot → `cu tell Finder` 改选中到 B →
  对工具栏按钮 ref 做 action → **必须 `stale_observation`**。
  **这条测试在 B1a 之前应当失败**（证明洞真实存在），之后通过。
  **不写这条测试就不要合 B1。**
- **B1b 强制回归（r3-#2）**：上面那条测试**在 B1b 与 B1c 之后必须继续通过** ——
  即"选中集合变化但 cardinality 不变"的场景不得被放行。这条是 B1b 的合入闸。
- **B1b 正向**：目标元素、祖先链、窗口选中集合都不变，窗口另一角有无关元素变化
  → action **成功**，响应带 `tree_drift: true`

---

### B2. 统一 ref 投影 → 折叠容器链

**优先级**: P1 · **工作量**: 2 天 · **风险**: 中高
**前置**: **X1 必须先合。**

**问题**

`cu snapshot Finder --limit 200`：`cell 72 · statictext 55 · row 29 · image 26 · textfield 15 · button 3`，
**既无 title 又无 value 的占 122/200 = 61%**。侧边栏一个条目产出 4 个 ref：

```
[1] row        ""         (258,202 231×32)
[2] cell       ""         (268,202 211×32)
[3] statictext "Recents"  (303,209 169×18)
[4] image      "clock"    (276,213 22×11)
```

`--limit 50` 用完走不出侧边栏。直接违反 CLAUDE.md Rule 3
（"Only interactive elements get refs. Static layout elements are skipped."）,
而 `INCLUDED_ROLES`（`ax.rs:680`）含 `AXRow`/`AXCell`/`AXImage`/`AXStaticText`。

### 装饰节点必须按 actions 判定，不能按 title（r3-#4）

v2 的规则**折叠不了自己举的这个例子**：
`image "clock"` **有 title**，不满足"无 title/value"条件；它又让 cell 产生分支，
不满足单子链条件 → 四个 ref 一个都消不掉。

**实测证据**（`cu why 4 --app Finder`）：

```json
{"role":"image","title":"clock",
 "actions_supported":[], "click_supported":false, "enabled":true}
```

→ **有 AXTitle ≠ 用户可操作。** 装饰节点的判据是 `actions_supported` 为空
且无 `AXPress`/`AXConfirm`/`AXOpen`。

### 改动（顺序不可颠倒）

**第 1 步 —— 按 actions 识别并排除装饰节点（需三态）**

判据：actions **确定为空** 且 role ∈ {image, statictext} 且非唯一标签来源 → 不占 ref。

> **"确定为空"不能用 `copy_action_names` 的返回值直接判断**（r5-#7）。
> `ax.rs:1173-1175` 的文档注释自陈：
> *"Empty vec means the element exposes no actions **or the call failed**"* ——
> `err != AX_OK` 与真正无 action 都返回空 `Vec`。
> 用它当删除依据，一次瞬时 AX 错误就会**静默删掉一个可操作元素**。

改为三态：

```rust
enum Actions { Known(Vec<String>), KnownEmpty, Unknown }
```

**只有 `KnownEmpty` 允许折叠**；`Unknown` 一律保守保留该 ref。
`AXActionNames` 需要一次额外 IPC，只对候选装饰节点查，不遍历全树。

**第 2 步 —— 在已排除装饰节点的树上判断可折叠链**

排除 `image "clock"` 后，`cell` 变为单子链 `row → cell → statictext`。

> **v4 的 "bbox 面积比 > 0.8" 折叠不了自己的示例**（r5-#5）。实算：
>
> | 链节 | 尺寸 | 面积 | 与父比 |
> |---|---|---|---|
> | `row` | 231×32 | 7392 | — |
> | `cell` | 211×32 | 6752 | **0.913** ✓ |
> | `statictext` | 169×18 | 3042 | **0.451** ✗ |
>
> 阈值卡在第二段，四个 ref 还是消不掉。

修正：**阈值只施加于容器 wrapper 之间**（`row`↔`cell` 这类同为容器的相邻节点），
**标签叶节点豁免面积比**——它本来就应该比容器小，
其归属由"单子链 + 是该链唯一标签来源"决定，不由面积决定。

折叠结果：role 取最外层可交互角色（`row`），title/value 取叶节点标签（`"Recents"`），
附 `collapsed_from: ["cell","statictext"]`。

**第 3 步 —— 裁定合成元素的 backing 归属**（r6-#4）

> v5 只写了"必须明确定义"，把裁决推给实现者 —— 与 C1 曾经的多选题是同一个毛病。
> 不同选择会改变 `perform` 的作用对象、焦点判定和 stale 比对，必须在计划内定死。

**裁定：backing = 链的最外层可交互节点（`row`）。** 各字段来源：

| 字段 | 取自 | 理由 |
|---|---|---|
| `backing`（`perform`/`AXPress` 的实际对象） | **`row`** | 它是承载 `AXPress`/`AXSelected` 的那个节点；对 `statictext` 发动作在列表 UI 里通常无效 |
| `role` | `row` | 与 backing 一致，避免"role 说 row 但动作打在 statictext" |
| `axPath` | **`row`** | 必须与 backing 同源，否则 `cu click <ref>` 与 `cu click --ax-path` 分叉（X0 清点 4 C2） |
| `x/y/width/height` | **`row`** | 坐标 fallback 要落在可点区域；`statictext` 的窄框会让点击错过行的空白部分 |
| `title` / `value` | **`statictext`**（链中唯一标签来源） | 这正是折叠的目的 —— 把标签提升到可操作节点上 |
| `selected` / `focused`（B1a 新增） | **`row`** | 选中态是行级语义；`statictext` 通常不暴露 `AXSelected` |
| `collapsed_from` | `["cell","statictext"]` | 可追溯性 |

**一句话规则**：**除 `title`/`value` 外，合成元素的一切都取自 backing。**
标签是唯一被"提升"的东西。

### 测试

- **X1 的五路 identity 等价性测试必须继续通过**
- **针对这个四节点实例的精确断言**（r3-#4）：
  Finder 侧边栏 "Recents" 条目在折叠后产出 **1 个 ref**，
  其 title/value 为 `"Recents"`，`collapsed_from` 含 `cell` 与 `statictext`
- **axPath 往返**：合成元素的 `axPath` 经 `descend_to_ax_path` 解析回同一真实元素
- **行为测试**：Finder 主目录 `--limit 50` **包含主文件列表至少一个条目**（当前做不到）
- 折叠后 `cu click <ref>` 实际选中的对象与折叠前点击最内层 statictext 一致
  （用 `cu tell Finder 'selection'` 读回，**不比坐标** —— 不同元素可能同坐标）

**⚠️ ref 编号会变** → **bump `INTERNAL_PROTOCOL`**（`broker.rs:23`，**3 → 4**；
B1a 已在 PR 5 把它从 2 提到 3，见 B1a）。
Observation TTL 5 分钟，跨版本复用会静默指错元素。
`diff.rs` 身份含 role，升级后首次 `--diff` 全量 added+removed，CHANGELOG 说明。

**验收**：Finder `--limit 50` 能看到文件列表；无标签元素占比 61% → ≤ 25%

---

### B3. 窗口身份贯穿全链路

**优先级**: P1 · **工作量**: 2–3 天 · **风险**: 中

**问题**：X0 清点 2 确认有 **5 类窗口选择机制**，其中 4 处在动作/校验路径上。
`cu window list` 显示 Finder 有 3 个窗口、VS Code 有 4 个 —— **多窗口是常态**，
但观测族只能看焦点窗口，想看第 2 个只能 `cu window focus`（项目自定义的 disruptive 操作）。

**改动**

1. `ax.rs` 抽出 `resolve_window(app_el, selector: WindowSelector) -> Option<CFTypeRef>`，
   `WindowSelector = Focused | Index(usize) | Id(u32)`
2. **机制 A 的 8 处全部改走它**（`ax.rs:421/1352/1572/1684/1734/1857/1984/2055`）
3. `--window <index>` 加到**每一个会解析窗口的命令**：

   | 组 | 命令 |
   |---|---|
   | 观测 | `Snapshot` `State` `Find` `Nearest` `ObserveRegion` `Why` |
   | 动作 | `Click` `SetValue` `Perform` `Scroll` `Hover` `Drag` |
   | **v3 漏掉的（r4-#3）** | **`Wait`**（`wait.rs:67` `ax::snapshot`）· **`Ocr`**（`ocr.rs:85` `screenshot::find_window`）· **`Screenshot`**（独立截图同样经 `find_window`）· **`click --text`**（`main.rs:3169` `ocr::recognize(pid)` → 读窗口 1） |

   > `click --text --window 2` 若不贯穿，会**在窗口 1 上 OCR、然后在窗口 2 的坐标系里点击**。

   **例外 —— `wait --new-window` 保持 app-scoped**：它调 `ax::window_count(pid)`
   （`ax.rs:1959`，读 `AXWindows` 数组长度），语义本就是"这个 app 是否多了窗口"，
   与"盯住某个窗口"正交。加 `--window` 对它无意义，应显式拒绝该组合并给出结构化错误。

4. **窗口 ID 必须贯穿六个环节**（v2 只列了五个，漏掉媒体捕获 —— r2-F5）：

| # | 环节 | 位置 |
|---|---|---|
| 1 | Observation 发布 | `broker.rs:836` `publish_observation` |
| 2 | Observation 校验 | `broker.rs:1828` `enforce_expected_observation` |
| 3 | ref 解析 | `ax.rs:1352` `resolve_ref` |
| 4 | 前后验证快照 | `main.rs:3090` `:4658` |
| 5 | 边界检查 | `ax.rs:1984` `window_bounds`（B4 依赖） |
| 6 | **媒体捕获** | **`screenshot.rs:332` `find_window`** —— 截图 / 标注 offset / capture-protection |

> 漏掉 #6 会产出**"窗口 2 的树 + 窗口 1 的截图"**。

5. **无条件删除启发式选窗**（r5-#2，v4 只禁了显式 `--window` 路径）

`screenshot.rs:418` `find_window_with_options` 按"面积最大的 layer-0"挑窗口
（`screenshot.rs:344-354`），正是 `AGENTS.md:228` Principle 1 明令禁止的做法。

> **v4 的范围划错了**：v4 只说"显式 `--window` 下禁用"，
> 但**默认 Focused 路径在 AX 失败后同样会落到 `screenshot.rs:347`**，
> 同样违反 `AGENTS.md:228`，并让 screenshot/OCR 对错误窗口返回 `ok:true`。
> 这条硬规则没有"仅在显式选择时生效"的版本。

规则（对**所有**路径，含默认 Focused）：
- window_id **必须**来自 AX 解析出的 `AXWindow`
- CGWindowList **只允许**按该 id 反查 sharing state（`screenshot.rs:396`）
- AX 给不出 window ID → **结构化错误，不回退**
- `find_window_with_options`（`screenshot.rs:418`）的"取面积最大 layer-0"整体删除

> X0 初稿曾提"让回退也能按索引选"作为备选 —— 该选项违反同一条规则，已删除。

**测试**
- **不要把 `--window 1` == 默认 focused 写成契约**（r5-#6）。
  `list_windows`（`ax.rs:465`）直接暴露 `AXWindows` 数组顺序，
  而该顺序**没有 API 保证**（实测 Finder/Code/Ghostty 三例恰好相符，是经验非契约）。
  在顺序不同的 app 上，把它断言成契约必然分叉。
  → **默认独立定义为 `WindowSelector::Focused`**（不是 `Index(1)`）；
  `--window N` 严格对应 `cu window list` 的第 N 项。两者是不同的选择器，不互为别名。
  测试断言"`cu window list` 的第 N 项"与"`--window N` 解析结果"一致，**不**断言 N=1 等于默认
- 开两个 Finder 窗口，`--window 1`/`--window 2` 的 `window_frame` 与元素集合不同
- **闭环**：`--window 2` snapshot → `--window 2` click，不 stale 且落在窗口 2
- **负向**：`--window 2` snapshot → `--window 1` click → 必须 `stale_observation`
- **树/截图同窗**：`--window 2 --with-screenshot` 的图像内容对应窗口 2
- AX 解析失败 + 显式 `--window` → 结构化错误，不静默回退
- **AX 解析失败 + 不传 `--window`（默认 Focused 路径）→ 同样结构化错误**（r7-#7）。
  v6 的负向用例只覆盖了显式路径，而**默认路径正是 `screenshot.rs:347` 回退发生的地方** ——
  只测显式路径等于没测到这条改动的主战场
- 越界索引 → `window_not_found`

---

### B4. click 边界预检

**优先级**: P1 · **工作量**: 1–2 天 · **风险**: 中
**前置**: B3 的 `resolve_window`

**问题 1**：`cmd_why` 算了 `in_window_bounds`（`main.rs:3770`），
但 `cmd_click`（`main.rs:3055`）**没有任何边界检查**。
滚出可视区的 row，AX 链失败后 fallback 到 `mouse::click`（`main.rs:3363`），
在窗口外坐标发事件。

**问题 2 —— v2 的修法有两个 bug**（r2-F6 / r3-#3）：

**(a) 不能持有裸 `AXUIElementRef`。**
递归遍历中的元素来自 `CFArrayGetValueAtIndex`，是**非 retained 引用**
（`AGENTS.md:212`: "All `AXUIElementCopyAttributeValue` results are +1 retained"；
`ax.rs:1327` 在父 `AXChildren` 释放后该指针即失效）。
项目已有的 `AxPathMatch`（`ax.rs:1437`）正是靠**保存整条数组所有权链**规避这一点：

```rust
struct AxPathMatch {
    element: CFTypeRef,
    /// AXChildren arrays, ordered from shallowest to deepest. They keep
    /// `element` alive until cleanup.
    owned: Vec<CFTypeRef>,
}
```

**(b) 重解析身份不能包含几何。** 滚动成功**本来就会改变几何**，
用"完整身份含旧几何"重解析会拒绝合法目标。

**改动**

**所有** ref / ax-path 的 CGEvent 路径共用同一预检 —— v5 只锚定了普通左键 fallback（r6-#3）：

| 分支 | 位置 | v5 是否覆盖 |
|---|---|---|
| ref 左键 AX 失败 fallback | `main.rs:3362` | ✅ |
| **ref 右键 / 双击** | **`main.rs:3352`** —— 走 `ax_find_element` 拿坐标后**直接** `mouse::click`/`double_click` | ❌ |
| **ax-path 右键 / 双击** | **`main.rs:3117`** —— 同上，经 `resolve_by_ax_path` | ❌ |
| ax-path 左键 fallback | `main.rs:3128` | ✅ |

右键/双击**根本不尝试 AX 动作链**，直接发坐标事件 —— 越界风险比左键更高，
因为它连"AX 成功就不需要坐标"这层保护都没有。

抽出共用的 `guard_coordinate_dispatch(pid, window, cx, cy, backing) -> Result<(f64,f64)>`，
上表四个分支全部经过它。在 CGEvent 派发之前：

1. 取**选定窗口**（B3 的 `resolve_window`，不是 `focused_window_geom`）的 bounds
2. 在界内 → 照常 fallback
3. 越界 → 用 **`AxPathMatch` 式 RAII 句柄**（或 `CFRetain` 后的独立所有权）
   持有已解析元素，对**该句柄**执行 `AXScrollToVisible`，
   再直接读它的新 `AXPosition`/`AXSize` —— **全程不按 ref 重新遍历**
   （滚动会 virtualize/重排 AX 树，同一整数 ref 可能指向另一个元素）
4. 句柄失效时的回退：**只比对滚动不应改变的语义身份**
   （role · title · value · axPath · AXIdentifier），**排除 x/y/width/height**；
   歧义时终止而非猜测
5. 仍越界或身份不符 → 结构化错误（hint 指向 `cu why`），**不发事件**

同样应用到 `--ax-path` 模式（`main.rs:3128`）。

**测试**
- **行为测试（断言目标行被选中，不是"UI 有变化"）**：
  Finder 列表滚到底部，对顶部已滚出视区的**具名文件** row 执行 click，
  `cu tell Finder 'name of item 1 of (get selection)'` 读回，**断言正是那个文件名**

- **六条路径 × 两种结局，共两组测试**（r8-#1）

  > v7 的测试写"所有越界场景→报错且不派发"，**与算法本身矛盾**：
  > 算法规定越界后先 `AXScrollToVisible`，**恢复成功就应该继续派发并命中**。
  > 按 v7 的测试实现，一个能滚动恢复的合法点击会被判为失败。

  **A 组 —— 可恢复越界 → 成功命中**（滚动后目标进入视区）

  | 路径 | 命令 |
  |---|---|
  | ref 左键 fallback | `cu click <ref>` |
  | ref 右键 | `cu click <ref> --right` |
  | ref 双击 | `cu click <ref> --double-click` |
  | ax-path 左键 fallback | `cu click --ax-path <p>` |
  | ax-path 右键 | `cu click --ax-path <p> --right` |
  | ax-path 双击 | `cu click --ax-path <p> --double-click` |

  断言：命令成功，且 `cu tell Finder 'name of item 1 of (get selection)'`
  读回的正是那个具名文件。

  **B 组 —— 恢复失败 → 不派发**（构造滚动后身份不符或仍越界）

  同样六条路径，断言：返回结构化错误，**且目标 app 状态不变**
  （用 `cu tell` 读回 selection / frontmost 对比）。
  只看退出码不够 —— 退出码非 0 也可能是事件已发出后才失败。

  **左键路径需要故障 seam**：Finder 的 row 很可能直接被 `AXSelected` 命中
  （AX 动作链第 8 步，`ax.rs:1095`），根本进不到 CGEvent fallback。
  测试必须用 env seam 强制左键的 AX 链失败，否则这两条**测的不是它声称的路径**。

  右键/双击**本来就不走 AX 动作链**（`main.rs:3352` / `:3117` 直接取坐标发事件），
  无需 seam —— 它们是越界风险最高的路径。

- 负向：构造滚动后 ref 指向不同元素的场景 → 结构化错误而非误点
- 视区内的正常路径不受影响

---

## 批次 C — 延迟

### C1. 统一 limit 语义 → 减少 click 的树遍历

**优先级**: P1 · **工作量**: 1–2 天 · **风险**: 中

**问题 —— `limit` 概念本身不自洽**（X0 清点 3）

一次 `cu click 5 --app X` 涉及 **5 个不同的元素基数**：

| 阶段 | 基数 | 来源 |
|---|---|---|
| 1. `pre_state` 快照 | **200** | click 的 `--limit` 默认（`main.rs:590`） |
| 2. Observation 校验快照 | **50** | `expected.limit`，来自 `cu snapshot` 默认（`main.rs:203`） |
| 3. ref 解析 | **无界** | `_limit` 被忽略（`ax.rs:1378/1664/1672/1724`） |
| 4. 动作后快照 | **200** | click 的 `--limit` |
| 5. 响应内嵌快照 | **50** | `ACTION_SNAPSHOT_ELEMENT_LIMIT`（`main.rs:4587`） |

> **这就是 v2 的 C1 会炸的原因**（r2-F4）：直接把 `pre_state`（200 元素）
> 喂给 enforcement（Observation 是 50 建的）→ `observed_generation` 必然不等
> → **所有 ref click 全挂**。C1 不能只做局部适配。

**改动 —— 单一模型，无可选项**

> v3 在这里留了三处"或"（r4-#4）。在"唯一可执行规范"里那是缺陷 ——
> 每个选项都会改变公开默认输出、可解析 ref 范围和遍历次数。v4 各选定一个。

**先更正 v4 的三处事实错误**（r5-#3）。实际默认值（脚本枚举 `main.rs`）：

| 默认 | 命令（行号） |
|---|---|
| **50** | `Snapshot`(203) · `Perform`(**294**) · `SetValue`(**330**) · `Why`(822) · `State`(856) |
| **200** | `ObserveRegion`(401) · `Nearest`(433) · `Find`(476) · **`Wait`(520)** · `Click`(590) |
| 20 | `Commands`(164)（记录条数，与元素无关） |
| **无** | **`Type` / `Key` 根本没有 `limit` 字段** |

v4 说"set-value/perform 由 200 降到 50"——**它们本来就是 50**；
v4 说"与 type/key 对齐"——**这两个命令没有该 flag**；
v4 **整条漏掉了 `Wait`(520)**，而它是 200。
（根因：X0 清点 3 记对了行号但把 294/330 错标成 type/key，错误传导进 v4。）

**决定 1 —— 不降任何默认值**（v5 的"4 个命令 200 → 50"已撤回）

> **v5 的降默认会让有效 ref 不可达**（r6-#2），不是"输出变小"这么轻。
>
> 本计划自己在 B2 里论证过：**Finder 的前 50 个 ref 走不出侧边栏**。
> 把 `Find`/`Nearest`/`ObserveRegion` 降到 50，等于让这些查询命令永远看不到主内容区。
>
> `Wait` 更严重。`wait.rs:50-53`：
>
> ```rust
> let effective_limit = match condition {
>     Condition::Gone(ref_id) if *ref_id > limit => *ref_id + 50,   // 只有 --gone 扩容
>     _ => limit,
> };
> ```
>
> `--ref` **不在扩容分支里**。`cu snapshot --limit 200` → `cu wait --ref 150`
> 在 limit=50 下永远找不到 ref 150 → **必然超时**。

保持现状默认值不变。改为解决真正的问题 —— **让上界跟随需求**：

`wait.rs:50` 的扩容规则从只覆盖 `Gone` 扩展到 **`Ref` 与 `Gone` 都适用**：

```rust
let effective_limit = match condition {
    Condition::Gone(ref_id) | Condition::Ref(ref_id) if *ref_id > limit => *ref_id + 50,
    _ => limit,
};
```

**跨边界行为测试**（r7-#2 更正）：

> v6 写的 `snapshot --limit 200` → `wait --ref 150 --timeout 5` **测不到新分支** ——
> `Wait` 的默认 limit 本来就是 200（`main.rs:520`），ref 150 < 200，改动前就能命中。
> 我刚在本节上方更正了默认值表，转头又写了一条与它矛盾的测试。

必须**显式压低 limit** 才能进入扩容分支：

```bash
cu wait --ref 150 --limit 50 --timeout 5     # 改动前必超时，改动后必命中
cu wait --gone 150 --limit 50 --timeout 5    # 对照组：已有扩容，前后都应命中
```

**决定 2 —— 解析器用 `expected.limit`，不用命令的 `--limit`**

> **v4 在这里自相矛盾**（r5-#3）：一边说"`_limit` 真正实现约束"，
> 一边说"命令 `--limit` 只控制动作后快照"。而 `main.rs:3361` 传给
> `ax_click` 的正是命令的 limit。两条同时成立时，
> `cu snapshot --limit 200` → `cu click 100` 会在 50 处停止解析 —— **本来可用的流程变必失败**。

单一模型：

| 用途 | 上界来源 |
|---|---|
| ref 解析（`ax_click`/`ax_find_element`/`ax_set_value`/`ax_perform`） | **`expected.limit`**（建立 Observation 时的 limit） |
| 动作前校验快照 | `expected.limit`（同上，见决定 3） |
| 动作后快照 | 命令的 `--limit` |
| 响应内嵌快照 | `ACTION_SNAPSHOT_ELEMENT_LIMIT = 50`（不变） |

`ax_click` 等四处的 `_limit` 参数**保留并真正使用**，由调用方传入 `expected.limit`。

> v7 说"删除参数"——**那样 resolver 就没有任何上界了**（r8-#2）。
> 上界的**来源**是 Observation，但**承载它的仍是这个参数**。删掉等于回到无界。

所有命令的 `--limit` 默认值保持现状（见上表）。

**边界测试**：`snapshot --limit 200` → `click 150`（ref > 50）必须成功解析。

**决定 3 —— enforcement 权威取快照，verification 复用它**

`enforce_expected_observation` 用 `expected.limit` 取一次快照并**返回给调用方**；
`cmd_click`/`cmd_set_value`/`cmd_perform`（`main.rs:2775/2847/3339`）
用这份作为 `pre_state`，不再自取。

不采用"按 `expected.limit` 截取 pre_state" —— 截断需要证明
"前 N 个元素与独立取 N 个元素相同"，在投影可能折叠子树（B2）之后这个前提并不显然。

*语义变化*：动作前的校验快照上界由 **Observation 的 limit** 决定，
而非命令的 `--limit`。命令的 `--limit` 只控制**动作后**快照。
需在 SKILL.md 与 `references/commands.md` 写明。

**已撤回**：`AXUIElementCopyElementAtPosition` hit-test 快速路径 ——
遮挡/分层下可能返回错误元素（Q1）。

**测量口径**（r2-F6）：计数器必须打在**遍历入口**，同时覆盖 `walk()` 与
X1 建立的 canonical iterator 的全部 5 个消费者 ——
只统计 `ax::snapshot()` 会漏掉 `resolve_ref`，得出"已降到 2"的假结论。

**验收**
- 遍历计数 **4 → 3**（撤回 hit-test 后 2 是达不到的）
- **AX 工作量上界测试**（r6-#6）：`limit=50` 的 snapshot 与解析 `ref 5` 时，
  visited-node 数与 batch-read 次数都必须有上界，**不得随目标树规模线性增长**。
  只验收"遍历入口 4→3"是不够的 —— 一次入口内部读整棵树同样是回归
- 端到端改善用 `scripts/bench-ab.py --via-broker` 实测后填入，**不预设百分比承诺**

---

### C2. 削减 Broker 固定开销

**优先级**: P1 · **工作量**: 1–2 天 · **风险**: 中

**实测**（交错 A/B，n=24，中位数）

| 命令 | 直连 | 经 Broker | 开销 |
|---|---|---|---|
| `snapshot Finder --limit 50` | 141.7 ms | 204.9 ms | **+63.2 ms (+45%)** |
| `apps` | 57.7 ms | 103.3 ms | **+45.6 ms (+79%)** |
| `find --app Finder --role button` | 167.3 ms | 214.2 ms | **+46.9 ms (+28%)** |

**固定税 ≈ +45–65 ms/命令，与命令种类无关**（`apps` 完全不碰 AX，开销同样 45 ms）。

**C2a — 每条命令 3 次 fsync**
`persist_record`（`broker.rs:383`）内 `sync_all()`（`:402`），
调用点 accept（`:1236`）→ dispatch（`:1512`）→ finish（`:1887`）。APFS 单次 1–10 ms。
→ `options.mutating == false` 时跳过 `sync_all()`。

**C2b — 20 ms 轮询粒度**
`broker.rs:1586` 在 `try_wait()` 循环里 → 平均白等 10 ms。
→ 独立线程阻塞 `child.wait()` + `mpsc::recv_timeout`；取消/超时检查仍按 20 ms 节奏。

> **必须先定义 `Child` 的所有权移交**（r4-#5）。v3 只写"移到线程"，但
> `terminate_child_group`（`broker.rs:1628`）要用 `child.id()` 杀进程组 ——
> `Child` 一旦 move 进 waiter 线程，协调线程就再也调不到它，
> 取消/超时路径直接失效，且可能留下孤儿进程组。

所有权契约：

1. **waiter 线程独占 `Child` 并负责 reap**（`wait()` 返回后经 channel 送出 exit status）
2. 协调线程在 spawn 后、move 之前**预存 PGID**（`setpgid(0,0)` 已在
   `pre_exec` 里做过，`broker.rs:1466`，所以 PGID == child pid）
3. 取消/超时时，协调线程**按预存 PGID 发信号**（`kill(-pgid, SIGKILL)`），
   **不碰 `Child`**
4. 发信号后仍**等待 waiter 的退出消息**再收尾，确保进程已被 reap 而非变僵尸

**新增测试**：cancel 与 timeout 两条路径各跑一次，断言
①命令返回后目标进程组内**无残留进程**（`ps -g <pgid>` 为空）②无僵尸进程。

**C2c — 父子进程重复 NSWorkspace 枚举**
父进程 `Cmd::resource()`（`main.rs:1034`）解析一次，子进程再解析一次（28 处调用点）。
→ 经 env 传下去（沿用 `EXPECTED_OBSERVATION` 机制）。

> **身份判据**（r2-F10）：**不能用 bundle_id** —— 目标进程退出、
> 同一 app 重启拿到同一 pid 时，bundle_id 校验照样通过。
> 用**进程实例身份**：pid + `NSRunningApplication.launchDate`。

**但取 launchDate 的方式决定了这条优化是否成立**（r8-#5）：

`running_apps_native()`（`system.rs:47`）会枚举**全部**运行中的 app。
子进程若靠它拿 launchDate，**枚举一次都没省下，C2c 的收益归零**。

必须新增按 PID 直查的 helper：

```rust
/// NSRunningApplication(processIdentifier:) —— 单进程查询，不枚举。
fn process_instance(pid: i32) -> Option<ProcessInstance>   // { pid, bundle_id, launch_date }
```

父进程把**完整解析结果**（pid + name + bundle_id + launch_date）经 env 传下去，
子进程只对这一个 pid 直查校验。

身份比对提为纯函数（供 D1 单测）：

```rust
fn same_process_instance(expected: &ProcessInstance, actual: &ProcessInstance) -> bool
```

**三组纯函数测试**：同 PID/同 launchDate → `true`；
**同 PID/不同 launchDate → `false`**；
launchDate 缺失 → **回退到二次解析**，不得静默放行。

**测试**
- `scripts/bench-ab.py --via-broker`（两 arm 独立 Broker home）测三条代表命令
- **崩溃恢复回归**：kill Broker 后 **mutating** 命令记录仍可经 `cu commands` 恢复
- **PID 复用回归** —— 见上方 C2c 的三组纯函数测试；
  **不要用"杀掉再重启"构造**（几乎必得新 PID，删掉 launchDate 校验也照样绿）

**验收**：固定税 ~50 ms → ≤ 25 ms

---

## 批次 D — 工程基础

### D1. 补纯逻辑单元测试

**优先级**: P0 · **工作量**: 2 天 · **风险**: 零

13,437 行只有 **2 个 Rust 单元测试**（`system.rs:798` `:837`）。
CI 的 hosted job 跑 `cargo test` **实质什么都没验证**，全部有效覆盖依赖自建 TCC runner。

**优先级最高的三个**（都是本计划其它条目的回归守卫）：

| 函数 | 位置 | 守护 |
|---|---|---|
| `broker_child_argv` | `main.rs:1308` | **A1** |
| `RefProjector::project` 全分支 | X1 新增 | **X1 / B2** |
| `focused_inside_webarea` / `same_focused_element` | `main.rs:2444` `:2428` | **A2** |
| `observed_generation` / `same_element` | `broker.rs:1751` `:1784` | **B1** |

**其余可纯单测的**：`parse_region`(`main.rs:4326`) · `method_meta`(`:35`) ·
`annotate_method`(`:70`) · `compact_action_text`(`:4590`) · `is_paste_app`(`:2406`) ·
`find_disruptive_applescript`(`:4248`) · `ocr_text_center_in_region`(`:4353`) ·
`parse_path_segment`(`ax.rs:1386`) · `build_path_segment`(`ax.rs:222`) ·
`normalize_role`/`is_included`(`ax.rs:704`/`:699`) · `diff::diff`/`content_changed` ·
`ErrorCode::classify`/`CuError::to_json` · `png_dimensions`(`file_result.rs:198`) ·
`reject_symlink_components`(`:171`) · `parse_pid_selector`(`system.rs:268`) ·
`resolve_target_from_apps`(`system.rs:311`) · `validate_client_key`/`validate_request_id` ·
`is_terminal` · key combo 解析 · `bench-ab.py` 的等价门禁 abort 路径

**新增**：**axPath 生成/解析往返测试**（X0 清点 4 C2）——
`build_path_segment` + `self_path` 拼接产出的 path，
经 `parse_path_segment` + `child_matches_segment` 必须解析回同一元素。

**注意**：`main.rs` 是 bin crate，测试写在同文件的 `#[cfg(test)] mod tests`。

**验收**：`cargo test` 断言数 ≥ 80；hosted CI job 成为有意义的 gate。

---

### D2. 定位 `tell` 套件的顺序依赖

**优先级**: P0（**必须最先做**） · **工作量**: 半天 · **风险**: 零

全量 `run_all.sh` → `831 passed, 1 failed`，`Failed suites: tell`；
单独跑 → `27 passed, 0 failed`。顺序依赖或 flaky。
汇总只输出 `Failed suites: tell`，**不输出是哪条断言**。

> 基线本身就有一个失败套件，而每个 PR 的 gate 要求"全绿" ——
> 不先修这个，所有 gate 都无法执行。

**改动**
- `helpers.sh` 的 `_fail` 累积失败断言到文件，`run_all.sh` 在 TOTAL 后打印明细
- 排查依赖。可疑方向：Broker per-app resource lock（`broker.rs:1351`）与前序套件竞争；
  或 Automation 权限首次授权对话框时序

**验收**：连续 3 次全量 `run_all.sh` 全绿；失败时汇总直接定位到断言。

---

## 明确不做

| 项 | 理由 |
|---|---|
| MCP server 模式 | CLAUDE.md 明令禁止 |
| `--timeout` 全局化 | 单位歧义（root 毫秒 vs 子命令秒）+ breaking（Q1） |
| `AXUIElementCopyElementAtPosition` hit-test | 遮挡/分层下可能返回错误元素（Q1） |
| CGWindowList 按索引回退 | 违反 `AGENTS.md:228` Principle 1（r3-#5） |
| B1c 之前放松任何默认门禁 | r1-F4 反例成立（Q2） |
| `cu type` 的 `unknown_outcome` 改退出码 | 保持非零 + `ok:false`（Q3） |
| 重写 Broker 为常驻长连接 | 改变"短命 CLI 进程"的公开边界 |

---

## 已裁决的开放问题

| 问题 | 裁决 |
|---|---|
| **Q1** `--timeout` | 只全局化 `--json`/`--human`/`--client-key`/`--request-id`；剥离逻辑按 flag 分开 |
| **Q2** `ax_generation` | 拒绝默认放松；先补 `AXSelected`/`AXFocused`（B1a），再收窄（B1b，**用身份集合非计数**），最后才谈放松（B1c） |
| **Q3** `unknown_outcome` | 保持非零退出 + `ok:false` |
| **Q4** 协议版本 | **B1a bump（2 → 3）**、**B2 再 bump（3 → 4）** —— 两者都改了 Observation 的结构或编号，同版本 Broker 复用会造成假性 stale；`machine_schema_version` 由 A2 单独处理（1.0 → 1.1），与协议版本不耦合 |
| **Q5** `DANGEROUS_FRONTMOST` | 独立安全项，改用 bundle ID；用户配置**只允许扩充保护列表，不允许缩减** |

---

## 合入顺序

```
PR 0   D2                    修 flaky —— 不先做，后面所有 gate 无法执行
PR 1   X1                    canonical iterator + 现存 ref 错位 bug
PR 2   D1                    单元测试网（含 project 全分支、axPath 往返）
PR 3   批次 A（A1 A3 A4 A5）   A5 在 X1 之上 rebase；等价测试放本 PR
PR 4   A2                    axPath 默认关闭（含 webarea + focus_verified 回归）
PR 5   B1a                   补 AXSelected/AXFocused + bump 协议 2→3 —— 只增严，可独立上线
PR 6   C1                    先统一 limit 语义，再合并遍历
PR 7   C2                    Broker 固定开销
PR 8   B3                    窗口身份贯穿六环节
PR 9   B4                    边界预检（依赖 B3 的 resolve_window）
PR 10  B2                    折叠容器链（依赖 X1；bump 协议 3→4）
PR 11  B1b + B1c             收窄 drift 模型（B1a 的选中漂移测试必须继续通过）
```

**每个 PR 的 gate**：
`cargo clippy --all-targets -- -D warnings` · `cargo fmt --check` · `cargo test` ·
`bash tests/commands/run_all.sh`

**gate 标准**：PR 0 完成前为**"不低于 831 passed 且本 PR 新增断言全绿"**；
PR 0 完成后收紧为**"全绿，0 failed"**。
