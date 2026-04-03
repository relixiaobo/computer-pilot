# Agent Computer-Use CLI 工具深度调研报告

> 调研时间：2026-04-02

---

## 一、市场全景：现有产品与项目

### 1. 商业产品（全桌面控制）

| 产品 | 公司 | 接口 | 桌面控制 | 浏览器控制 | 核心方法 | OSWorld 分数 |
|------|------|------|---------|-----------|---------|-------------|
| **Computer Use** | Anthropic | REST API | Yes | Via desktop | 截图 + 视觉模型 | 72.7% (Opus 4.6) |
| **Operator / CUA** | OpenAI | ChatGPT / API | No | Yes | GPT-4o 视觉 + RL | 38.1% |
| **Project Mariner** | Google | Chrome 扩展 / Cloud | No | Yes | Gemini 2.0 视觉 | ~70% (Mind2Web) |
| **Copilot Computer Use** | Microsoft | Copilot Studio | Yes | Yes | 视觉 + UIA | Preview |
| **Manus AI** | Meta (收购) | Web/Desktop/扩展 | Yes | Yes | 云端 Agent | 86.5% (GAIA) |
| **ACT-2** | Adept (→Amazon) | Enterprise API | Yes | Via desktop | 像素级感知 | - |
| **Screen Agent** | UiPath | Studio 平台 | Yes | Yes | 视觉 + RPA | 53.6% (Verified) |

### 2. CLI 编码 Agent

| 工具 | 接口 | 桌面控制 | 开源 | 许可证 |
|------|------|---------|------|--------|
| **Claude Code** (Anthropic) | Terminal/IDE | No (代码/文件) | No | Proprietary |
| **Open Interpreter** | Terminal/SDK | Yes (via 代码执行) | Yes | AGPL-3.0 |
| **Aider** | Terminal (Git集成) | No | Yes | Apache-2.0 |
| **OpenCode** | Terminal/IDE | No | Yes | MIT |
| **Gemini CLI** (Google) | Terminal | No | Yes | Apache-2.0 |
| **Codex App** (OpenAI) | CLI/Web | Sandbox | No | Proprietary |

### 3. 计算机控制 MCP Server（直接相关）

| 项目 | 平台 | 能力 | GitHub |
|------|------|------|--------|
| **MCPControl** | Windows | 鼠标/键盘/窗口/截图 | claude-did-this/MCPControl |
| **Windows-MCP** | Windows | 文件导航/应用控制/UI交互 | CursorTouch/Windows-MCP |
| **computer-use-mcp** | 跨平台 | 完整计算机控制 | domdomegg/computer-use-mcp |
| **computer-control-mcp** | 跨平台 | PyAutoGUI + OCR | AB498/computer-control-mcp |
| **mcp-remote-macos-use** | macOS | 远程 macOS 控制 | baryhuang/mcp-remote-macos-use |
| **macOS Automator MCP** | macOS | 200+ 自动化配方 | steipete/macos-automator-mcp |
| **Microsoft MCP on Windows** | Windows | 官方 MCP 集成 | learn.microsoft.com |

### 4. 专注 CLI 的 Computer-Use 项目（最相关）

| 项目 | 描述 | GitHub |
|------|------|--------|
| **CLI-Anything** (HKUDS) | 用 CLI 替代截图式 GUI 操作，直接 CLI 调用应用 | HKUDS/CLI-Anything |
| **OpenCLI** | 将任何网站/Electron应用/二进制转为 CLI 供 agent 使用 | jackwener/opencli |
| **Cua** (YC) | 开源 computer-use 基础设施：沙箱 + SDK + Benchmark | trycua/cua |
| **open-computer-use** (Coasty) | SOTA 开源 computer-use agent，82% OSWorld-Verified | coasty-ai/open-computer-use |
| **UI-TARS Desktop** (字节) | 开源多模态 agent，本地+远程桌面/浏览器控制 | bytedance/UI-TARS-desktop |

### 5. 浏览器自动化 Agent

| 项目 | 接口 | 方法 | 开源 |
|------|------|------|------|
| **Playwright MCP** (Microsoft) | MCP/CLI | 无障碍树（非视觉） | Yes (Apache-2.0) |
| **browser-use** | Python SDK | Agent-Browser-LLM 循环 | Yes (MIT) |
| **Stagehand** (Browserbase) | TypeScript SDK | Playwright + AI | Yes (MIT) |
| **Skyvern** | Python/TS/API | 视觉 + Playwright | Yes (AGPL-3.0) |

### 6. Agent 框架

| 框架 | 语言 | MCP 支持 | 特点 |
|------|------|---------|------|
| **LangChain / LangGraph** | Python/TS | Yes | 750+ 工具集成，图编排 |
| **CrewAI** | Python | Yes | 多 agent 角色协作 |
| **AG2** (原 AutoGen) | Python | Yes | 事件驱动，跨框架互操作 |
| **Google ADK** | Python/TS/Go | Yes | Gemini 优化，模块化 |
| **AutoGPT** | Python | No | 167K+ stars，自主目标追求 |

---

## 二、技术方案深度分析

### 1. 屏幕感知方案对比

| 方案 | 速度 | 通用性 | 精确度 | Token 消耗 | 适用场景 |
|------|------|--------|--------|-----------|---------|
| **截图 + 视觉模型** | 慢 (2-10s) | 极高（任何应用） | 中（坐标不精确） | 高 | 通用兜底 |
| **无障碍树 (A11y)** | 快 (<500ms) | 中（依赖应用支持） | 高（确定性坐标） | 低 | 现代应用优先 |
| **OCR** | 快 | 中 | 中 | 低 | 补充层 |
| **视觉元素标注 (VAG)** | 中 | 高 | 高（按ID引用） | 中 | 消除坐标问题 |

**推荐策略：瀑布式降级**
```
结构化 API → 无障碍树 → 视觉模型（兜底）
```

### 2. 动作执行：各平台方案

#### macOS
| 工具 | 类型 | 能力 | 推荐度 |
|------|------|------|--------|
| **CGEvent** | 原生 API | 低级鼠标/键盘事件 | 性能最佳 |
| **AXUIElement** (Accessibility API) | 原生 API | UI 元素发现 + 语义交互 | 最强大 |
| **AppleScript / JXA** | 脚本 | 应用控制、UI Scripting | 应用集成 |
| **cliclick** | CLI 工具 | 鼠标点击/移动、键盘事件 | CLI 快捷方案 |
| **screencapture** | 系统命令 | 截图 | 标配 |

#### Windows
| 工具 | 类型 | 能力 |
|------|------|------|
| **UI Automation (UIA)** | 原生 API | WinForms/WPF/Qt/浏览器/Store 应用 |
| **Win32 API** | 原生 API | Handle-based 控制 |
| **pywinauto** | Python 封装 | 同时支持 Win32 和 UIA |

#### Linux
| 工具 | 类型 | X11 | Wayland |
|------|------|-----|---------|
| **xdotool** | CLI | Yes | No |
| **ydotool** | CLI | Yes | Yes (uinput) |
| **AT-SPI2** | D-Bus | Yes | Yes |

> **Wayland 碎片化问题**：Wayland 出于安全移除了 X11 的大部分 API，每个合成器有自己的 API。ydotool 能处理输入但无法管理窗口。

#### 跨平台
| 工具 | 语言 | 特点 |
|------|------|------|
| **nut.js** | Node.js | 最全面：鼠标/键盘/截图/无障碍树/图像匹配 |
| **pyautogui** | Python | 成熟稳定，无 a11y 支持 |
| **robotjs** | Node.js | 已被 nut.js 替代 |

### 3. 架构模式

#### 核心 Agent 循环：See → Think → Act → Observe

```
1. CAPTURE:  截图 / 读取无障碍树 / 获取屏幕状态
2. INJECT:   发送视觉状态 + 任务上下文 + 历史记录到 LLM
3. REASON:   LLM 分析状态，规划下一步动作
4. ACT:      执行鼠标/键盘/API 动作
5. OBSERVE:  捕获新状态，验证动作是否成功
6. REPEAT:   直到任务完成或达到最大迭代次数
```

#### 架构选型

| 模式 | 描述 | 优势 | 代表 |
|------|------|------|------|
| **Tool-Based** | LLM 调用工具定义，执行层实现 | 结构清晰，可控 | Anthropic Computer Use |
| **MCP-Based** | 通过 MCP 动态发现能力 | 可扩展，互操作 | 各 MCP Server |
| **ReAct** | 推理与行动交替进行 | 适应不可预测的 UI | Agent S2 |
| **Code Generation** | LLM 生成代码直接执行 | 灵活，利用现有 API | Open Interpreter |

### 4. 关键技术挑战与解法

| 挑战 | 问题 | 解法 |
|------|------|------|
| **坐标系统** | Retina/HiDPI 缩放、图像降采样后坐标偏移 | 自行缩放截图 + 反向映射坐标；校准例程 |
| **延迟** | 每步 >10s（视觉模型） | 视觉 diff（仅变化时截图）、前缀缓存、宏序列、优先用 a11y 树 |
| **动态 UI** | 点击过快 UI 未渲染、弹窗干扰 | wait-for-element 轮询、动作延迟、弹窗检测 |
| **安全沙箱** | Agent 以用户权限执行 | MicroVM (Firecracker) > gVisor/Docker > Seatbelt/Bubblewrap |
| **跨平台** | 各平台 API 完全不同 | 统一 ScreenController 接口 + 平台特定后端 |

---

## 三、前沿研究与趋势

### 1. 关键 Benchmark 现状 (2026年初)

| Benchmark | 最高分 | Agent | 人类基线 |
|-----------|--------|-------|---------|
| OSWorld (主榜) | **72.7%** | Claude Opus 4.6 | ~72% |
| OSWorld-Verified | **75.0%** | GPT-5.4 | ~72% |
| WebArena | **~60%** | Top agents | - |
| AndroidWorld | **50%** | Agent S2 | - |

> 两年内 OSWorld 从 ~12% 跃升到 ~75%，已匹配/超过人类水平。

### 2. 重要研究论文

| 论文/项目 | 年份 | 贡献 |
|----------|------|------|
| **OSWorld** (NeurIPS 2024) | 2024 | 首个可扩展真实计算机 benchmark |
| **ShowUI** (CVPR 2025) | 2025 | 2B 参数开源 VLA 模型，90% 更少幻觉动作 |
| **UFO → UFO2 → UFO3** (Microsoft) | 2024-2025 | 双 agent → AgentOS → 多设备协调 |
| **UI-TARS-2** (字节跳动) | 2025 | RL 训练的多模态 agent，支持游戏/代码/工具 |
| **Agent S2** (Simular) | 2025 | Mixture-of-Grounding + 层次规划，多项 SOTA |
| **SeeClick** (ACL 2024) | 2024 | 纯截图 GUI agent + ScreenSpot benchmark |
| **CogAgent** | 2024 | 18B VLM 专用于 GUI 理解 |

### 3. 行业六大趋势

1. **能力内化**：Computer use 从独立 demo 变为产品标配功能（Operator 并入 ChatGPT，Claude 集成桌面控制）
2. **混合 GUI + API**：UFO2、Agent S2 证明视觉 + 原生 API 混合方案比纯截图更可靠
3. **CLI 作为互补通道**：CLI-Anything 等项目证明 CLI 交互可避免像素级 GUI 控制的脆弱性
4. **MCP 成为通用协议**：97M+ 月下载量，5800+ 社区 Server，所有主要厂商支持
5. **开源基础设施成熟**：Cua、open-computer-use、UI-TARS Desktop 提供生产级沙箱环境
6. **多设备/多 Agent 协调**：UFO3 Constellation、Mariner 10并发任务

### 4. 主要未解决问题

- **效率**：最好的 agent 仍比人类多 1.4-2.7x 步骤
- **安全**：对抗攻击（Fine-Print Injection）可导致级联失败
- **泛化**：在一个 OS/应用上训练的 agent 难以迁移到新环境
- **长期规划**：多步骤分支任务的错误恢复仍不可靠
- **动态内容**：视频、动画、加载状态困扰当前 VLM agent

---

## 四、方案推荐

### 推荐架构

```
+------------------+     +------------------+     +-------------------+
|   CLI Interface  |     |   Agent Core     |     | Platform Backend  |
|  (Ink/React TUI) | --> | (ReAct Loop)     | --> | (per-OS impl)     |
|                  |     |                  |     |                   |
| - 用户输入       |     | - LLM 推理       |     | macOS:            |
| - 动作展示       |     | - 工具选择       |     |  CGEvent + AX API |
| - 确认审批       |     | - 状态追踪       |     |  + screencapture  |
| - 流式输出       |     | - 错误恢复       |     |                   |
+------------------+     | - 检查点         |     | Linux:            |
                         +------------------+     |  ydotool + AT-SPI |
                                |                 |                   |
                         +------------------+     | Windows:          |
                         |   MCP Server     |     |  UIA + Win32      |
                         | (暴露工具能力)    |     +-------------------+
                         +------------------+
```

### 技术栈推荐

| 层面 | 推荐 | 理由 |
|------|------|------|
| **语言** | TypeScript (Node.js / Bun) | Claude Code 验证了此选择；nut.js 提供跨平台桌面自动化；Ink 提供终端 UI |
| **桌面自动化** | nut.js + 平台特定回退 | 单一 API 覆盖鼠标/键盘/截图/无障碍树 |
| **终端 UI** | Ink (React for terminals) | Claude Code 已验证；组件化；流式支持 |
| **LLM 集成** | Anthropic SDK (Computer Use tool) | 专为此场景设计；72.7% OSWorld |
| **感知策略** | 混合：无障碍树优先 + 视觉兜底 | 速度 + 可靠性 + 通用性 |
| **Agent 模式** | ReAct + 显式检查点 | 最适合不可预测的 UI 环境 |
| **工具暴露** | MCP Server | 未来互操作；任何 MCP 客户端可使用 |
| **沙箱** | MicroVM（不受信）/ Container（开发） | 最强隔离，不牺牲开发速度 |
| **运行时** | Bun | 比 Node.js 快；原生 TS 支持 |

### 实施路线图

```
Phase 1: 单平台 MVP (macOS)
├── 截图捕获 (screencapture)
├── 鼠标/键盘控制 (CGEvent / cliclick)
├── 基础 Agent 循环 (截图 → LLM → 动作)
└── CLI 交互界面 (Ink)

Phase 2: Agent 智能
├── ReAct 模式 + Anthropic Computer Use API
├── 动作验证（前后截图对比）
├── 错误恢复机制
└── 迭代次数限制 + 检查点

Phase 3: 感知增强
├── 无障碍树集成 (AXUIElement)
├── 混合感知策略（a11y 优先，视觉兜底）
├── 视觉元素标注 (element numbering)
└── 坐标校准例程

Phase 4: 生态集成
├── MCP Server 封装
├── 多 LLM 支持 (LiteLLM 模式)
├── 插件/工具扩展机制
└── 安全沙箱

Phase 5: 跨平台
├── Linux 后端 (ydotool + AT-SPI)
├── Windows 后端 (UIA + Win32)
├── 统一 ScreenController 接口
└── 平台检测 + 自动适配

Phase 6: 高级能力
├── 宏录制/回放
├── 多步骤任务编排
├── 多 Agent 协调
└── 云端沙箱执行
```

### 最值得参考的项目

| 项目 | 参考价值 |
|------|---------|
| **CLI-Anything** | 核心理念最接近——用 CLI 替代 GUI 截图操作 |
| **macOS Automator MCP** | macOS 平台能力暴露的最佳参考（200+ 配方） |
| **Cua** | 沙箱基础设施 + SDK 设计参考 |
| **Claude Code** | CLI Agent 架构、权限系统、终端 UI 的金标准 |
| **Open Interpreter** | CLI Agent 循环 + 多语言代码执行参考 |
| **Anthropic Computer Use** | 截图-动作循环的 API 设计参考 |
| **Playwright MCP** | 无障碍树优先方案的参考（4x 省 token） |

---

## 五、核心洞察

1. **CLI > GUI 截图**：CLI-Anything 证明，对于有 CLI 接口的应用，直接 CLI 调用比截图识别更快、更准、更省 token。你的工具应该优先发现和使用应用的 CLI 接口。

2. **MCP 是必选项**：97M+ 月下载量，所有主要厂商支持。将工具能力通过 MCP 暴露是最佳互操作策略。

3. **混合感知是正确方向**：纯视觉太慢太贵，纯 a11y 覆盖不全。微软 UFO2 和 Agent S2 都验证了混合方案。

4. **macOS 优先有利**：macOS 的 Accessibility API 最完善，CGEvent 性能最佳，且你当前在 macOS 环境。先做好一个平台再扩展。

5. **安全是差异化机会**：当前开源项目普遍缺乏安全考虑。内置权限确认、沙箱、操作审计可成为核心卖点。
