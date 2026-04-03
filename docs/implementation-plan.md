# Computer Pilot — Implementation Plan

> 当前项目的唯一实施基线。后续实现、任务拆分和交接都以本文为准。

---

## 文档定位

- 本文是 **source of truth**
- 只记录 **已确认** 的 v1 范围、协议、任务和注意事项
- 研究、备选方案、跨平台探索统一归档到 `docs/archive/`
- 如实施过程中结论变化，应先更新本文，再改代码
- 注意：本文后半段仍保留部分旧的 daemon / Swift 设计草案，当前交接与实现应优先以前面的 `当前状态`、`项目进展`、`交接说明` 为准

## 当前状态

- 项目状态：**已开始实现**
- 当前交付目标：**macOS CLI v1**
- 当前实现范围：`cu status`、`cu apps`、`cu snapshot`、`cu click`、`cu type`、`cu key`、`cu screenshot`、`cu ocr`、`cu system`、`cu script`
- 明确不做：MCP server、跨平台后端、recipe/learn、自学习体系
- 当前已落地命令：`cu status`、`cu apps`、`cu snapshot`
- 当前原生实现：`native/src/main.rs` + `native/src/ax.rs`（Rust helper + AX 模块）
- 当前 transport：CLI 直接调用 Rust helper 的 **one-shot 子命令**，读取 stdout JSON
- 后台 daemon / Unix socket 设计目前 **不是实际代码路径**；若未来恢复，需先更新本文再改代码
- 当前 `snapshot` 的运行策略：
  - **优先使用 Rust AX helper** — 通过 macOS Accessibility API 遍历真实 UI 元素树
  - 失败时回退到 Node 侧 `System Events` 快照
  - 输出包含：role、title、value、position (x/y)、size (width/height)、ref ID
  - 支持 `--app` 指定应用、`--limit` 限制元素数量

## 接手顺序

1. 先看 [docs/README.md](/Users/lixiaobo/Documents/Coding/computer-pilot/docs/README.md)
2. 再通读本文的 `当前状态`、`核心设计决策`、`Daemon API 设计`、`实施计划与任务清单`
3. 需要背景时，再去 `docs/archive/`

## 项目进展

| 项目 | 状态 | 备注 |
|------|------|------|
| 文档入口整理 | 已完成 | `docs/README.md` 负责交接入口 |
| 主计划收敛 | 已完成 | 本文已去掉未确认的 v1 承诺 |
| 研究文档归档 | 已完成 | 已移动到 `docs/archive/` |
| TypeScript CLI 骨架 | 已完成 | `src/` 已具备基础命令、输出和 helper 调用逻辑 |
| Rust helper 骨架 | 已完成 | `native/` 已能返回 `health/apps/snapshot` |
| Node 侧 snapshot fallback | 已完成 | `src/system-events-snapshot.ts` 已接入 CLI |
| mock smoke tests | 已完成 | `tests/run.sh` 覆盖 `status/apps/snapshot` |
| Rust AX snapshot | 已完成 | `native/src/ax.rs` 真实 AX 树遍历，输出 role/title/value/position/size |
| 真实 helper 冒烟验证 | 已完成 | `status/apps/snapshot` 均已稳定；snapshot 已优先走 Rust AX helper |
| click 命令 | 已完成 | `cu click <ref>` 和 `cu click <x> <y>`，通过 CGEvent 发送鼠标事件 |
| screenshot 命令 | 已完成 | `cu screenshot [app]` 静默窗口截图 + `--full` 全屏模式，返回坐标偏移 |
| key 命令 | 已完成 | `cu key <combo> --app <name>`，通过 System Events 发送快捷键到指定 app |
| type 命令 | 已完成 | `cu type <text> --app <name>`，通过 System Events 输入文字 |

## 交接说明

### 已验证可用

- `cu status`
- `cu apps`
- `cu snapshot` — 真实 AX 树遍历，返回可用 UI 元素（role、title、value、position、size、ref ID）
- `cu click <ref>` — snapshot + ref 解析 + CGEvent 点击
- `cu click <x> <y>` — 直接坐标点击
- mock 路径下的 `cu snapshot`、`cu click`（smoke tests 通过）
- 从仓库外目录运行 CLI 时，helper 路径解析已不依赖 `cwd`

### 当前未完成

- `system/script` 命令（非 v1 核心）

### 当前技术判断

- **核心操作闭环已完成**：snapshot → click / key / type / screenshot
- 所有动作命令支持 `--app` 指定目标应用
- key/type 使用 AppleScript System Events 确保可靠送达目标 app
- screenshot 使用 AX window bounds + `screencapture -x -R`，静默、窗口级

### 推荐的下一步

1. 发布 v1 — 当前能力已支持完整的桌面自动化流程
2. 实现 MCP server — 让 AI agent 直接调用 cu 命令
3. 优化 snapshot — 增加 AXHelp/AXIdentifier 等更多元素信息

## 项目结构

```
computer-pilot/
├── package.json                      # npm 包配置 (cu CLI)
├── tsconfig.json
├── tsup.config.ts
├── .gitignore
├── README.md
│
├── src/                              # TypeScript CLI 源码
│   ├── cli.ts                        # 命令行入口 (commander)
│   ├── helper-client.ts              # Rust helper 调用封装
│   ├── helper-runtime.ts             # helper 路径解析 / client 构造
│   ├── system-events-snapshot.ts     # snapshot 的临时 Node fallback
│   ├── output.ts                     # 输出格式化 (JSON/human)
│   └── paths.ts                      # 版本 / 包根目录常量
│
├── native/                           # Rust helper 源码
│   ├── Cargo.toml
│   ├── Cargo.lock
│   └── src/
│       └── main.rs                   # helper 入口 (`health/apps/snapshot`)
│
├── bin/                              # 预编译 Rust helper
│   ├── desktop-helper-arm64          # Apple Silicon
│   └── desktop-helper-x86_64         # Intel Mac（待产出）
│
├── tests/
│   └── run.sh                        # mock smoke tests
│
└── docs/
    ├── README.md                     # 文档入口 / 交接说明
    ├── implementation-plan.md        # 当前唯一实施基线
    └── archive/                      # 研究与过程性文档（非 source of truth）
        ├── research.md
        ├── deep-research.md
        ├── implementation-principles.md
        ├── macos-control-bible.md
        ├── cross-platform-strategy.md
        └── market-landscape-2026.md
```

---

## 核心设计决策

### 决策 1: 当前 transport — CLI 直接调用 Rust helper

```
CLI (TypeScript)                    Rust helper
    │                                   │
    │  spawn: `desktop-helper apps`     │
    │ ─────────────────────────────────▶│
    │  stdout JSON                      │
    │ ◀─────────────────────────────────│
```

**当前确认：**
- 现有代码使用 one-shot helper，避免后台 daemon 管理复杂度
- 每次 CLI 调用都会前台执行一次原生 helper
- helper 输出结构化 JSON，CLI 负责参数解析和 human/json 输出
- `snapshot` 目前是例外：CLI 会先尝试 Node 侧 `System Events` 快照，再回退到 Rust scaffold

**保留说明：**
- 文档后续仍保留 daemon / Unix socket 设计草案，作为候选演进方向
- 在它重新成为实施目标前，不视为当前代码真相

### 决策 2: 命令体系 — 6 核心 + 3 扩展

**核心命令 (LLM 必须知道的):**

| 命令 | HTTP | 说明 |
|------|------|------|
| `cu apps` | `GET /apps` | 列出运行中的应用 |
| `cu snapshot "App"` | `POST /snapshot` | AX tree 快照 + ref 编号 |
| `cu click <ref\|x,y>` | `POST /click` | 点击元素 (auto-snapshot) |
| `cu type <ref\|-> "text"` | `POST /type` | 输入文本 (auto-snapshot) |
| `cu key <combo>` | `POST /key` | 键盘快捷键 (auto-snapshot) |
| `cu screenshot` | `GET /screenshot` | 截图 (base64 或文件路径) |

**扩展命令 (LLM 按需发现):**

| 命令 | HTTP | 说明 |
|------|------|------|
| `cu system <action>` | `POST /system` | 系统操作 (volume, dark-mode, open, notify) |
| `cu script <app> <action>` | `POST /script` | AppleScript 语义操作 (send mail, create event...) |
| `cu ocr [--region x,y,w,h]` | `GET /ocr` | 截图 + OCR 文字识别 |

**辅助命令:**

| 命令 | HTTP | 说明 |
|------|------|------|
| `cu setup` | `GET /setup` | 权限引导 |
| `cu permissions` | `GET /permissions` | 权限状态检查 |
| `cu status` | `GET /health` | Daemon 状态 |

### 决策 3: Snapshot 格式 — 带 Section Hint 的紧凑文本

```
[app] Finder — "文稿" (3 windows)

  toolbar:
[1] button "后退"
[2] button "前进" (disabled)
[3] button "搜索"
  sidebar:
[4] row "收藏" (selected)
[5] row "桌面"
[6] row "文稿"
  content:
[7] cell "项目A" (folder)
[8] cell "报告.pdf"
[9] cell "照片.jpg"
  statusbar:
[10] text "3 个项目"
```

**规则:**
- 第一行：`[app] 应用名 — "窗口标题"`
- Section hint（`toolbar:`, `sidebar:`, `content:` 等）仅在检测到 AXSplitGroup/AXToolbar/AXScrollArea 时添加
- 每个可交互元素一行：`[ref] role "name" [状态标注]`
- 状态标注：`(disabled)`, `(focused)`, `(selected)`, `value="..."`, `checked`
- 默认上限 50 个元素（`--limit` 可调）

### 决策 4: 安全模型 — 与 browser-pilot 对齐

```
所有写操作需要 daemon 在运行
所有读操作无副作用
权限由 macOS TCC 系统管理
Daemon 自身不存储敏感信息
```

---

## Daemon API 设计

### v1 边界（已确认）

- 本节只定义 **CLI v1 已确认** 的协议；未确认能力不写入 plan
- `snapshot` 只针对 **单个目标窗口** 生成 refs，不做跨窗口混合快照
- `section` 只是 best-effort hint，可为空；不能作为动作路由的硬依赖
- `click/type/key` 的 `ok=true` 必须表示 **动作已被验证成功**，不能只表示“尝试过”
- `permissions` 只暴露 **v1 实际使用到** 的权限检查
- `/system` 只收录 **已确认纳入 v1** 的能力；Wi-Fi、亮度等暂不写入协议

### 只读端点

```
GET  /health                        → {"ok":true, "platform":"macos", "version":"0.1.0"}
GET  /apps                          → {"apps":[{"name":"Finder","pid":123,"active":true,"scriptable":true},...]}
GET  /permissions                   → {"accessibility":true, "screenRecording":false}
GET  /screenshot                    → {"image":"base64...", "width":1440, "height":900}
GET  /screenshot?path=/tmp/ss.png   → {"path":"/tmp/ss.png", "width":1440, "height":900}
GET  /ocr                           → {"texts":[{"text":"File","x":10,"y":5,"w":30,"h":15},...]]}
GET  /ocr?region=100,100,400,300    → {"texts":[...]}  (局部 OCR)
```

**说明：**
- `inputMonitoring` 不在 v1 中暴露，因为当前 plan 未包含录制/学习功能
- 后续若新增依赖其它权限的 CLI 命令，再扩展 `/permissions` 和 `cu setup`

### 感知端点

```
POST /snapshot
  body: {"app":"Finder", "limit":50}
  resp: {
    "ok": true,
    "app": "Finder",
    "window": "文稿",
    "windowRef": "frontmost",
    "elements": [
      [1, "button", "后退", null, "toolbar"],
      [2, "button", "前进", null, "toolbar", "disabled"],
      [3, "textfield", "搜索", "", "toolbar"],
      [4, "row", "桌面", null, "sidebar"],
      [5, "cell", "报告.pdf", null, "content"]
    ]
  }
```

**元素数组格式:** `[ref, role, name, value, section, ...flags]`
- 比对象格式节省 ~40% tokens
- role 去掉 "AX" 前缀并小写化
- flags: "disabled", "focused", "selected", "checked"
- refs 的作用域限定为本次快照对应的单个窗口；动作后的 refs 必须全部重新生成
- `section` 是展示辅助信息，不保证完整、不保证稳定，缺失时返回 `null`

### 动作端点 (全部 auto-snapshot)

```
POST /click
  body: {"target":"3"}                     # ref-based
  body: {"target":"500,300"}               # coordinate-based
  resp: {
    "ok": true,
    "action": "click",
    "strategy": "ax-press",
    "verified": true,
    "note": "Refs below are NEW.",
    "app": "Finder",
    "window": "搜索结果",
    "elements": [[1, "textfield", "搜索", "报告", "toolbar", "focused"], ...]
  }

POST /type
  body: {"target":"3", "text":"hello"}     # type into ref 3
  body: {"target":"-", "text":"hello"}     # type at current focus
  body: {"target":"3", "text":"hello", "clear":true}  # clear then type
  body: {"target":"3", "text":"hello", "submit":true} # type then Enter

POST /key
  body: {"combo":"cmd+c"}
  body: {"combo":"Return"}
  body: {"combo":"cmd+shift+s"}
```

**动作成功语义（v1 确认）**
- `ok=true` 仅表示动作已通过 post-check 验证成功
- `ok=false` 表示所有已实现策略都未能确认成功，响应中应包含 `error` 和 `hint`
- `strategy` 只回传实际成功的策略名，不暴露未实现或未验证的“预期步骤”
- auto-snapshot 使用动作后的前台窗口重新生成 refs；不得复用旧 ref map

### 系统端点

```
POST /system
  body: {"action":"volume", "mode":"get"}
  body: {"action":"volume", "mode":"set", "level":50}
  body: {"action":"volume", "mode":"mute"}
  body: {"action":"dark-mode", "mode":"on"}
  body: {"action":"dark-mode", "mode":"off"}
  body: {"action":"dark-mode", "mode":"toggle"}
  body: {"action":"open", "target":"https://google.com"}
  body: {"action":"open", "target":"/path/to/file.pdf"}
  body: {"action":"notify", "title":"Done", "body":"Task completed"}
```

**v1 只包含以上 4 类 system action。**
- Wi-Fi、亮度等虽然可以研究，但本 plan 不写入 v1 契约
- `action/value` 的自由拼接不再作为协议设计，避免 CLI/daemon 两端各自解释

### 脚本端点

```
POST /script
  body: {"app":"Mail", "action":"send", "to":"x@y.com", "subject":"Hi", "body":"Hello"}
  body: {"app":"Safari", "action":"get-url"}
  body: {"app":"Chrome", "action":"exec-js", "code":"document.title"}
  body: {"app":"Calendar", "action":"create-event", "title":"会议", "date":"2026-04-03 14:00"}
  body: {"app":"Finder", "action":"tag", "path":"/file.pdf", "tag":"Important"}
  body: {"app":"raw", "code":"tell app \"Finder\" to ..."}  # raw AppleScript
```

### 管理端点

```
GET  /setup                         → 引导权限设置 (返回每一步的指令)
POST /shutdown                      → 关闭 daemon
```

---

## 实施计划与任务清单

### 总体原则

- 先打通最小 observe-act 闭环，再扩展能力
- 每个阶段都必须有对应的 CLI 验收方式
- 未经验证的能力不写成“默认可用”
- `snapshot`、`click`、`type` 的正确性优先级高于覆盖面

### Phase 0: 项目脚手架

**任务**
- 建立 `src/`、`native/`、`tests/` 基础目录
- 建立 TypeScript 构建链路和 Swift Package
- 定义 CLI 与 daemon 共享的协议模型
- 约定日志目录、socket 路径、daemon 生命周期

**注意事项**
- 先定协议，再写 handler
- 不要先写“临时 JSON”，避免后续双端各自兼容
- 从这一步开始就保留结构化日志

**TypeScript 端:**
```json
// package.json
{
  "name": "computer-pilot-cli",
  "version": "0.1.0",
  "type": "module",
  "bin": { "cu": "dist/cli.js" },
  "scripts": {
    "build": "tsup",
    "dev": "tsup --watch",
    "build:native": "cd native && swift build -c release",
    "postinstall": "node scripts/install-native.js"
  },
  "dependencies": {
    "commander": "^13.1.0"
  },
  "devDependencies": {
    "tsup": "^8.0.0",
    "typescript": "^5.7.0",
    "@types/node": "^22.0.0"
  }
}
```

**Swift 端:**
```swift
// native/Package.swift
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ComputerPilot",
    platforms: [.macOS(.v13)],  // macOS 13+ for ScreenCaptureKit
    products: [
        .executable(name: "desktop-helper", targets: ["ComputerPilot"])
    ],
    targets: [
        .executableTarget(
            name: "ComputerPilot",
            path: "Sources/ComputerPilot",
            linkerSettings: [
                .linkedFramework("ApplicationServices"),  // AXUIElement
                .linkedFramework("CoreGraphics"),          // CGEvent
                .linkedFramework("ScreenCaptureKit"),      // Screenshots
                .linkedFramework("Vision"),                // OCR
                .linkedFramework("Carbon"),                // Key codes
            ]
        )
    ]
)
```

**文件清单:**

| 文件 | 行数 (估算) | 说明 |
|------|------------|------|
| `src/cli.ts` | ~250 | 命令定义、参数解析、输出格式化 |
| `src/helper-client.ts` | ~80 | one-shot helper 调用封装 |
| `src/helper-runtime.ts` | ~60 | helper 路径解析 / 可执行文件检测 |
| `src/output.ts` | ~50 | emit/fail/emitSnapshot 输出函数 |
| `src/paths.ts` | ~10 | 路径常量 |
| **TS 合计** | **~450** | |
| `native/.../main.swift` | ~100 | Daemon 入口、HTTP 服务器、请求分发 |
| `native/.../Server.swift` | ~80 | Unix socket HTTP 服务器 |
| `native/.../Router.swift` | ~120 | 5 层路由决策 |
| `native/.../AXEngine.swift` | ~400 | AX tree 遍历、ref 分配、15-step click chain |
| `native/.../ScriptEngine.swift` | ~200 | AppleScript/JXA 模板、执行 |
| `native/.../EventEngine.swift` | ~150 | CGEvent 鼠标/键盘 |
| `native/.../VisionEngine.swift` | ~100 | ScreenCaptureKit + Vision OCR |
| `native/.../SystemEngine.swift` | ~100 | 系统 CLI 调用映射 |
| `native/.../Snapshot.swift` | ~80 | 快照数据结构、格式化 |
| `native/.../AXHelpers.swift` | ~100 | AX API 便捷封装 |
| `native/.../Permissions.swift` | ~60 | 权限检查 |
| `native/.../ProcessUtils.swift` | ~40 | 进程列表、前台应用 |
| **Swift 合计** | **~1530** | |
| **总计** | **~2000** | |

### Phase 1: Swift Daemon 核心

**任务**
- 实现 daemon 启动、探活、重连
- 实现 `/health`
- 实现 Unix socket HTTP server
- 建立统一错误返回模型

**注意事项**
- 先保证 CLI 能稳定拉起 daemon，再做业务能力
- 错误要区分：未启动、不可连接、超时、权限不足、响应非法

**1.1 HTTP 服务器 (Server.swift)**

基于 Foundation 的轻量 HTTP over Unix socket：

```swift
import Foundation

class HTTPServer {
    let socketPath: String
    private var fileHandle: FileHandle?

    init(socketPath: String) {
        self.socketPath = socketPath
    }

    func start(handler: @escaping (HTTPRequest) -> HTTPResponse) {
        // 删除旧 socket
        unlink(socketPath)

        // 创建 Unix socket
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutablePointer(to: &addr.sun_path.0) { ptr in
            socketPath.withCString { src in
                strcpy(ptr, src)
            }
        }

        // bind + listen
        withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { addrPtr in
                bind(fd, addrPtr, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        listen(fd, 5)
        chmod(socketPath, 0o600)

        // Accept loop
        DispatchQueue.global().async {
            while true {
                let clientFd = accept(fd, nil, nil)
                if clientFd < 0 { continue }
                DispatchQueue.global().async {
                    self.handleConnection(fd: clientFd, handler: handler)
                }
            }
        }
    }

    private func handleConnection(fd: Int32, handler: (HTTPRequest) -> HTTPResponse) {
        // 简化的 HTTP 解析 (足够用于 localhost IPC)
        // ... 读取请求行、headers、body
        // ... 调用 handler
        // ... 写回 HTTP 响应
        close(fd)
    }
}
```

**备选方案:** 使用 Swift 的 `NWListener`（Network framework），更现代但更复杂。对于 IPC 场景，原始 socket 足够。

**1.2 AX Engine — 核心 (AXEngine.swift)**

```swift
import ApplicationServices

class AXEngine {
    // 16 种可交互角色
    static let interactiveRoles: Set<String> = [
        "AXButton", "AXCheckBox", "AXRadioButton",
        "AXTextField", "AXTextArea", "AXSecureTextField",
        "AXPopUpButton", "AXComboBox", "AXSlider",
        "AXMenuItem", "AXLink", "AXCell", "AXRow",
        "AXTab", "AXDisclosureTriangle", "AXIncrementor"
    ]

    // Section 检测 — 通过 AX 角色推断 UI 区域
    static let sectionRoles: [String: String] = [
        "AXToolbar": "toolbar",
        "AXSplitGroup": "",  // 需要分析子元素
        "AXScrollArea": "",  // 由位置推断
        "AXTabGroup": "tabs",
        "AXMenuBar": "menubar",
        "AXSheet": "dialog",
        "AXGroup": ""  // 需要上下文
    ]

    struct ElementInfo {
        let element: AXUIElement
        let role: String
        let name: String?
        let value: String?
        let position: CGPoint?
        let size: CGSize?
        let enabled: Bool
        let focused: Bool
        let section: String?
    }

    /// 只对目标窗口生成快照；不跨窗口混合 refs
    func snapshot(pid: pid_t, limit: Int = 50) throws -> [ElementInfo] {
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 3.0)  // 3 秒超时

        guard let window = resolveTargetWindow(app) else {
            throw SnapshotError.noUsableWindow
        }

        var refs: [ElementInfo] = []

        func walk(_ element: AXUIElement, depth: Int, inheritedSection: String?, maxDepth: Int = 15) {
            guard depth < maxDepth, refs.count < limit else { return }

            let attrs = batchRead(element)
            let role = attrs["AXRole"] as? String ?? ""
            let section = inferSection(role: role, attrs: attrs) ?? inheritedSection

            // v1: section 只用于展示，允许为空；动作逻辑不能依赖它
            if Self.interactiveRoles.contains(role) {
                let name = (attrs["AXTitle"] as? String)
                    ?? (attrs["AXDescription"] as? String)
                    ?? ""
                if !name.isEmpty || attrs["AXValue"] != nil {
                    refs.append(ElementInfo(
                        element: element,
                        role: role,
                        name: name.isEmpty ? nil : name,
                        value: attrs["AXValue"] as? String,
                        position: attrs["AXPosition"] as? CGPoint,
                        size: attrs["AXSize"] as? CGSize,
                        enabled: (attrs["AXEnabled"] as? Bool) ?? true,
                        focused: (attrs["AXFocused"] as? Bool) ?? false,
                        section: section
                    ))
                }
            }

            guard let children = getChildren(element) else { return }
            let nextDepth = (role == "AXGroup" && attrs["AXTitle"] == nil)
                ? depth : depth + 1
            for child in children {
                walk(child, depth: nextDepth, inheritedSection: section, maxDepth: maxDepth)
            }
        }

        walk(window, depth: 0, inheritedSection: nil)
        return refs
    }

    /// 批量读取属性 (3-5x faster than individual reads)
    private func batchRead(_ element: AXUIElement) -> [String: Any] {
        let keys: [CFString] = [
            kAXRoleAttribute, kAXTitleAttribute, kAXValueAttribute,
            kAXDescriptionAttribute, kAXPositionAttribute, kAXSizeAttribute,
            kAXEnabledAttribute, kAXFocusedAttribute, kAXChildrenAttribute
        ]
        var values: CFArray?
        AXUIElementCopyMultipleAttributeValues(element, keys as CFArray, 0, &values)

        var result: [String: Any] = [:]
        guard let vals = values as? [Any?] else { return result }
        for (i, key) in keys.enumerated() {
            if i < vals.count, let val = vals[i] {
                result[key as String] = val
            }
        }
        return result
    }

    /// v1 不在 plan 中锁死“15 个具体步骤”；
    /// 只锁定：AX-first，多策略回退，且每一步都必须经过验证。
    func performClick(element: AXUIElement, baseline: ElementInfo) throws -> ClickResult {
        let strategies: [ClickStrategy] = [
            .axPress,
            .axConfirm,
            .axOpen,
            .axPick,
            .setSelected,
            .focusThenPress,
            .coordinateClick
        ]

        for strategy in strategies {
            if tryAttempt(strategy, on: element) {
                if verifyClickSucceeded(strategy: strategy, baseline: baseline, element: element) {
                    return ClickResult(strategy: strategy.rawValue, verified: true)
                }
            }
        }

        throw ActionError.unverifiedClick
    }

    // ... helper methods: tryAction, trySetAttribute, getChildren, etc.
}
```

**这里刻意不写死的内容：**
- 不把“15-step”逐条写成 v1 必须实现的承诺；具体回退链以 spike 和集成测试结果为准
- 不把 `section`、父表格/父行推断、祖先点击等启发式写成必选实现
- 不把 `CGEvent` 回退定义为成功；必须经过 `verifyClickSucceeded(...)` 确认

**1.3 Event Engine (EventEngine.swift)**

```swift
import CoreGraphics

class EventEngine {
    static let shared = EventEngine()

    func click(at point: CGPoint, button: CGMouseButton = .left) { ... }
    func doubleClick(at point: CGPoint) { ... }
    func drag(from: CGPoint, to: CGPoint) { ... }
    func scroll(deltaY: Int, deltaX: Int = 0) { ... }
    func pressKey(code: CGKeyCode, flags: CGEventFlags = []) { ... }
    func pressCombo(_ combo: String) { ... }
    func typeText(_ text: String) { ... }

    // 组合键解析
    static let keyMap: [String: CGKeyCode] = [
        "a": 0, "s": 1, "d": 2, "f": 3, /* ... */
        "return": 36, "tab": 48, "space": 49, "delete": 51,
        "escape": 53, "up": 126, "down": 125, "left": 123, "right": 124
    ]

    static let modifierMap: [String: CGEventFlags] = [
        "cmd": .maskCommand, "command": .maskCommand,
        "shift": .maskShift,
        "alt": .maskAlternate, "option": .maskAlternate,
        "ctrl": .maskControl, "control": .maskControl
    ]
}
```

### Phase 2: TypeScript CLI

**任务**
- 实现 `cu status`、`cu apps`、`cu snapshot`
- 实现 `ensureHelper()`、human/json 双输出、统一报错
- 接入 `/permissions` 与 `cu setup`

**注意事项**
- `snapshot` 必须只针对单窗口
- 不做跨快照 ref 恢复；动作后统一生成新 refs
- `section` 只用于展示，不能用于动作路由

**2.1 CLI 入口 (cli.ts) — 完全对齐 browser-pilot 模式**

```typescript
import { Command } from 'commander';
import { HelperClient } from './helper-client.js';
import { ensureHelper } from './helper-runtime.js';
import { emit, fail, emitSnapshot, useJson } from './output.js';

const program = new Command();
program
  .name('cu')
  .description('Control your desktop from the command line')
  .version('0.1.0')
  .option('--human', 'force human-readable output')
  .addHelpText('after', `
Workflow:
  cu apps                           # list running apps
  cu snapshot "Finder"              # see UI with [ref] numbers
  cu click 3                        # click [3] — returns updated snapshot
  cu type 3 "hello"                 # type into [3] — returns updated snapshot
  cu key cmd+s                      # keyboard shortcut — returns updated snapshot
  cu screenshot                     # capture screen (visual fallback)

Refs:
  snapshot/click/type/key return interactive elements like:
    [1] button "Save"  [2] textfield "Name"  [3] checkbox "Agree"
  Use the number: cu click 1, cu type 2 "hello"
  Refs refresh after every action.

Extended:
  cu system volume 50               # system control
  cu script "Mail" send --to "..." --subject "..." --body "..."
  cu ocr                            # screenshot + OCR text extraction
  cu setup                          # permission setup wizard

Full reference: cu help <command>
`);

// ── Core commands ──────────────────────────────────

program.command('apps')
  .description('List running applications')
  .action(action(async () => {
    const helper = await ensureHelper();
    const data = await helper.apps();
    if (useJson()) emit(data);
    else {
      for (const app of data.apps) {
        const flags = [
          app.active ? '★' : ' ',
          app.scriptable ? 'S' : ' '
        ].join('');
        console.log(`${flags} ${app.name} (pid ${app.pid})`);
      }
    }
  }));

program.command('snapshot')
  .argument('[app]', 'application name (default: frontmost)')
  .option('--limit <n>', 'max elements', '50')
  .description('Get AX tree snapshot with [ref] numbers')
  .action(action(async (app, opts) => {
    const helper = await ensureHelper();
    const data = await helper.snapshot({ app, limit: parseInt(opts.limit) });
    emitSnapshot(data);
  }));

program.command('click')
  .argument('<target>', 'ref number or x,y coordinates')
  .description('Click element — returns updated snapshot')
  .action(action(async (target) => {
    const helper = await ensureHelper();
    const data = await helper.click({ target });
    emitSnapshot(data);
  }));

program.command('type')
  .argument('<target>', 'ref number or - for current focus')
  .argument('<text>', 'text to type')
  .option('--clear', 'clear field before typing')
  .option('--submit', 'press Enter after typing')
  .description('Type text into element — returns updated snapshot')
  .action(action(async (target, text, opts) => {
    const helper = await ensureHelper();
    const data = await helper.type({
      target, text, clear: opts.clear, submit: opts.submit
    });
    emitSnapshot(data);
  }));

program.command('key')
  .argument('<combo>', 'key combo (e.g. cmd+c, Return, Escape)')
  .description('Press keyboard shortcut — returns updated snapshot')
  .action(action(async (combo) => {
    const helper = await ensureHelper();
    const data = await helper.key({ combo });
    emitSnapshot(data);
  }));

program.command('screenshot')
  .option('--path <path>', 'save to file instead of stdout')
  .description('Capture screen (fallback for non-AX apps)')
  .action(action(async (opts) => {
    const helper = await ensureHelper();
    const data = await helper.screenshot({ path: opts.path });
    if (opts.path) emit({ ok: true, path: data.path, width: data.width, height: data.height });
    else emit(data);
  }));

// ── Extended commands ──────────────────────────────

program.command('system')
  .argument('<action>', 'system action (volume, dark-mode, open, notify)')
  .argument('[value...]', 'action parameters')
  .description('System-level control (no GUI needed)')
  .action(action(async (act, values) => {
    const helper = await ensureHelper();
    let body: any;

    if (act === 'volume' && values[0] === 'get') {
      body = { action: 'volume', mode: 'get' };
    } else if (act === 'volume' && values[0] === 'mute') {
      body = { action: 'volume', mode: 'mute' };
    } else if (act === 'volume' && /^\d+$/.test(values[0] ?? '')) {
      body = { action: 'volume', mode: 'set', level: parseInt(values[0], 10) };
    } else if (act === 'dark-mode' && ['on', 'off', 'toggle'].includes(values[0])) {
      body = { action: 'dark-mode', mode: values[0] };
    } else if (act === 'open' && values.length > 0) {
      body = { action: 'open', target: values.join(' ') };
    } else if (act === 'notify' && values.length > 0) {
      body = { action: 'notify', title: values[0], body: values.slice(1).join(' ') };
    } else {
      throw new Error('Unsupported `cu system` usage in v1 plan');
    }

    const data = await helper.system(body);
    emit(data);
  }));

program.command('script')
  .argument('<app>', 'application name')
  .argument('<action>', 'action to perform')
  .option('--to <email>', 'recipient (for mail)')
  .option('--subject <text>', 'subject (for mail/event)')
  .option('--body <text>', 'body content')
  .option('--title <text>', 'title (for event/note)')
  .option('--date <date>', 'date (for event)')
  .option('--code <js>', 'JavaScript code (for browser)')
  .option('--path <path>', 'file path')
  .option('--tag <tag>', 'tag name')
  .description('App-specific scripted actions (AppleScript/JXA)')
  .action(action(async (app, act, opts) => {
    const helper = await ensureHelper();
    const data = await helper.script({ app, action: act, ...opts });
    emit(data);
  }));

program.command('ocr')
  .option('--region <x,y,w,h>', 'OCR specific region')
  .description('Screenshot + OCR text extraction')
  .action(action(async (opts) => {
    const helper = await ensureHelper();
    const data = await helper.ocr({ region: opts.region });
    emit(data);
  }));

// ── Setup ──────────────────────────────────────────

program.command('setup')
  .description('Guided permission setup wizard')
  .action(action(async () => {
    const helper = await ensureHelper();
    const perms = await helper.permissions();
    const steps = [
      { name: 'Accessibility', ok: perms.accessibility,
        fix: 'System Settings > Privacy & Security > Accessibility > Add your terminal app' },
      { name: 'Screen Recording', ok: perms.screenRecording,
        fix: 'System Settings > Privacy & Security > Screen & System Audio Recording > Add your terminal app' },
    ];
    for (const step of steps) {
      console.log(`${step.ok ? '✓' : '✗'} ${step.name}`);
      if (!step.ok) console.log(`  → ${step.fix}`);
    }
    if (steps.every(s => s.ok)) {
      console.log('\nAll permissions granted. Ready to use!');
    } else {
      console.log('\nGrant the permissions above, then run `cu setup` again.');
    }

    console.log('\nThis v1 setup only checks permissions required by current commands.');
  }));

program.command('status')
  .description('Check native helper status')
  .action(action(async () => {
    const helper = await ensureHelper();
    const data = await helper.health();
    emit(data);
  }));

program.parse();
```

**2.2 Helper Client (helper-client.ts) — 当前 one-shot helper 模式**

```typescript
export class HelperClient {
  async health() {
    return this.runJson(['health']);
  }

  async apps() {
    return this.runJson(['apps']);
  }

  async snapshot(options: { app?: string; limit?: number }) {
    const args = ['snapshot'];
    if (options.app) args.push('--app', options.app);
    if (options.limit) args.push('--limit', String(options.limit));
    return this.runJson(args);
  }
}
```

### Phase 3: 核心动作与验证

**任务**
- 实现 `click`、`type`、`key`
- 为 `click` 建立 post-check 验证
- 为 `type` 建立 value/focus 验证
- 实现动作后的 auto-snapshot

**注意事项**
- `ok=true` 必须表示动作已验证成功
- 不把“15-step 全量链路”作为首批硬目标，先做最小稳定策略集
- `CGEvent` 坐标回退不是成功条件，验证成功才算成功

### Phase 4: 感知补充、系统命令与测试

**任务**
- 实现 `screenshot`、`ocr`
- 实现 `/system` 的已确认子命令：`volume`、`dark-mode`、`open`、`notify`
- 实现 `script` 的最小可用能力
- 补齐 smoke test 和关键验收测试

**注意事项**
- OCR 和 system 命令不应反向扩大 v1 范围
- `script` 先做少量高确定性 action，不先铺大量模板
- plugin/skill 文档不是 CLI v1 的阻塞项，可放在主链路完成后补

### Phase 5: 可选集成

**任务**
- 如有需要，再补 `plugin/`、SKILL 文档、agent 集成说明

**注意事项**
- 可选集成不能改变 CLI v1 的核心协议
- 接手者应先完成 CLI 核心链路，再处理外围集成

**3.1 集成测试**

```bash
#!/bin/bash
# tests/run.sh
set -e

echo "=== cu apps ==="
cu apps

echo "=== cu snapshot Finder ==="
cu snapshot "Finder" --limit 10

echo "=== cu screenshot ==="
cu screenshot --path /tmp/cu-test.png && echo "Screenshot saved"

echo "=== cu system volume ==="
ORIG=$(cu system volume get | jq -r '.value')
cu system volume 30
cu system volume "$ORIG"

echo "=== cu key cmd+tab ==="
cu key "cmd+tab"

echo "All tests passed!"
```

**测试边界（v1 确认）**
- 以上脚本只作为 smoke test，不代表功能验收完成
- 真正的验收至少还需要：单窗口 ref 稳定性、动作失败返回、click post-check、type 后值验证、权限缺失提示
- 在这些验证补齐前，不在 plan 中写任何准确率或“几乎都能点到”的承诺

**3.2 SKILL.md (可选集成草稿，不阻塞 CLI v1)**

```markdown
---
name: computer-pilot
description: >
  Control desktop applications via the `cu` CLI tool. Use when the user needs to
  interact with desktop apps, manage files, control system settings, or automate
  any macOS workflow. Works with any application — not just browsers.
---

# Computer Pilot

Control the user's Mac desktop via bash commands. Every action returns a snapshot
of interactive elements with `[ref]` numbers you can use in follow-up commands.

## Prerequisites

- Run `cu setup` once to grant permissions (Accessibility + Screen Recording)
- If `cu` is not found: `npm install -g computer-pilot-cli`

## Core Workflow

```bash
cu apps                          # see what's running
cu snapshot "Finder"             # see app UI with [ref] numbers
cu click 3                       # click [3] — returns updated snapshot
cu type 5 "hello"                # type into [5] — returns updated snapshot
cu key cmd+s                     # keyboard shortcut — returns updated snapshot
```

## Understanding Snapshots

Every action returns interactive elements grouped by section:

```
[app] Finder — "文稿"
  toolbar:
[1] button "Back"
[2] textfield "Search" (focused)
  content:
[3] cell "report.pdf"
[4] cell "photo.jpg"
```

Use `[ref]` number in subsequent commands. Refs refresh after every action —
always use refs from the LATEST snapshot.

## Commands Quick Reference

| Command | When to use |
|---------|-------------|
| `cu apps` | See what apps are running |
| `cu snapshot "App"` | See app UI elements |
| `cu click <ref>` | Click a button, link, row, etc. |
| `cu type <ref> "text"` | Type into a text field |
| `cu key cmd+c` | Press keyboard shortcut |
| `cu screenshot` | Visual fallback (when snapshot is not enough) |
| `cu system volume 50` | System control (no GUI needed) |
| `cu script "Mail" send --to "..." --subject "..." --body "..."` | App scripting |
| `cu ocr` | Read text from screen via OCR |

## Patterns

### Read-then-act
```bash
cu snapshot "Finder"     # see what's there
cu click 3               # act on what you see
# response includes new snapshot — no need to call snapshot again
```

### System tasks (no GUI needed)
```bash
cu system volume 50
cu system wifi connect "OfficeNet"
cu system dark-mode on
cu system open "https://example.com"
```

### App scripting (reliable, no clicking)
```bash
cu script "Mail" send --to "user@example.com" --subject "Report" --body "See attached."
cu script "Safari" get-url
cu script "Calendar" create-event --title "Meeting" --date "2026-04-03 14:00"
```
```

---

## 实施状态与完成标准

### 当前建议的实施顺序

1. `status` + daemon 启动链路
2. `apps`
3. `snapshot`
4. `click`
5. `type`
6. `key`
7. `screenshot` / `ocr`
8. `system`
9. `script`
10. 可选 plugin / skill 集成

**MVP 完成标志:** 以下工作流可以端到端执行：

```bash
cu setup                              # ✓ 权限引导
cu apps                               # ✓ 列出应用
cu snapshot "Finder"                   # ✓ 看到 Finder UI
cu click 3                            # ✓ 点击按钮，返回新状态
cu type 1 "test.txt"                  # ✓ 输入文本
cu key Return                         # ✓ 确认
cu system volume 30                   # ✓ 系统控制
cu script "Safari" get-url            # ✓ AppleScript
cu screenshot                         # ✓ 截图
```

Agent 端验证：Claude Code 通过 SKILL.md 加载后，能使用 `cu` 命令自主完成 "在 Finder 中创建一个名为 Project 的文件夹" 这样的任务。

### 验收前必须确认的事项

- `snapshot` 在 Finder、Safari、System Settings 上都只返回单窗口 refs
- `click` 对成功和失败都有明确 post-check 结果
- `type` 能确认值变化，而不是只发送了键盘事件
- 缺少权限时，CLI 能返回明确的 `error` 和 `hint`
- smoke test 与关键验收测试均已补齐
