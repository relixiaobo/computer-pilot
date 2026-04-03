# Computer Pilot — 实现原理详解

> 从操作系统底层 API 到上层 Agent 交互，逐层拆解每一个技术细节。

---

## 目录

1. [整体架构：为什么是 TypeScript + Swift 双层结构](#1-整体架构)
2. [IPC 通信桥：CLI 进程与 Daemon 如何对话](#2-ipc-通信桥)
3. [Layer 0: 系统 CLI/API 引擎](#3-layer-0-系统-cliapi-引擎)
4. [Layer 1: AppleScript/JXA 语义引擎](#4-layer-1-applescriptjxa-语义引擎)
5. [Layer 2: AX Tree 引擎（核心）](#5-layer-2-ax-tree-引擎)
6. [Layer 3: CGEvent 输入合成引擎](#6-layer-3-cgevent-输入合成引擎)
7. [Layer 4: 视觉引擎（截图 + OCR）](#7-layer-4-视觉引擎)
8. [智能路由器：如何选择控制层](#8-智能路由器)
9. [Ref 系统：元素编号的分配与解析](#9-ref-系统)
10. [Daemon 状态管理](#10-daemon-状态管理)
11. [Auto-Snapshot：动作后自动返回状态](#11-auto-snapshot)
12. [安全引擎](#12-安全引擎)
13. [自学习：Recipe 录制与回放](#13-自学习系统)
14. [MCP Server 实现](#14-mcp-server-实现)

---

## 1. 整体架构

### 为什么是 TypeScript CLI + Swift Daemon？

```
用户终端 / Agent
     │
     │  bash: cu click 3
     │
     ▼
┌─────────────────────────────────────┐
│  TypeScript CLI (cu)                │  ← npm install -g computer-pilot
│  职责：                              │
│  - 解析命令行参数                    │
│  - 与 daemon 通信 (Unix socket)      │
│  - 格式化输出 (JSON/human)           │
│  - 自动启动 daemon                   │
│  运行时：Bun                         │
│  生命周期：每条命令启动一个进程，完成即退 │
└──────────────┬──────────────────────┘
               │ Unix Domain Socket
               │ JSON-RPC over socket
               ▼
┌─────────────────────────────────────┐
│  Swift Daemon (desktop-helper)      │  ← 预编译 arm64 + x86_64 二进制
│  职责：                              │
│  - 调用所有 macOS 原生 API            │
│  - 维护状态 (ref map, AX cache)      │
│  - 执行 5 层控制逻辑                  │
│  生命周期：持久运行，首次 cu 调用时启动  │
│  大小：< 15MB                        │
└─────────────────────────────────────┘
               │
               │ 直接调用 macOS 框架
               ▼
┌─────────────────────────────────────┐
│  macOS 系统层                        │
│  AXUIElement │ CGEvent │ SCKit      │
│  Vision      │ osascript │ ...      │
└─────────────────────────────────────┘
```

### 为什么不用 Rust / Zig / Node native addon？

| 方案 | 优势 | 劣势 |
|------|------|------|
| **Rust + objc2** (axcli, agent-desktop) | 性能好，类型安全 | macOS API 需要 FFI 桥接层，每个 API 都要手动绑定 |
| **Zig + @cImport** (usecomputer) | 极轻量，跨平台编译 | 生态小，AX/AppleScript 支持几乎为零 |
| **Node native addon** | 与 TypeScript 同进程 | 用户需要编译，安装失败率高 |
| **Swift 独立进程** (我们的选择) | 直接调用所有 macOS API，零 FFI | 需要 IPC，多一次进程间通信 |

Swift 的核心优势：**AXUIElement、CGEvent、ScreenCaptureKit、Vision、AppleScript Bridge 全部是一等公民**，无需任何桥接代码。IPC 开销 <10ms，相比 LLM 推理的 1-5s 可以忽略。

### 为什么是 Daemon 而不是每次命令启动新进程？

Agent 调用 `cu` 是一系列快速连续的命令：
```
cu snapshot "Finder"   →  看到 [3] 是 "新建文件夹" 按钮
cu click 3             →  点击它
cu type - "项目资料"    →  在弹出的对话框里输入名称
cu key Return          →  确认
```

如果每条命令都启动/关闭 Swift 进程：
- AX tree 缓存丢失，每次重新遍历 (50-300ms)
- Ref map 丢失，需要重建
- ScreenCaptureKit 权限检查每次重做
- 冷启动 Swift 进程本身 ~200ms

Daemon 模式下：
- 首次启动 ~500ms，之后每次 IPC <10ms
- AX tree 缓存复用
- Ref map 持久化在内存中
- 权限状态已检查过

这和 browser-pilot 的 daemon 架构完全一致（`bp` 命令通过 Unix socket 与持久化 daemon 通信）。

---

## 2. IPC 通信桥

### 协议设计：JSON-RPC over Unix Domain Socket

```
Socket 路径: /tmp/computer-pilot-{uid}.sock
```

**请求格式 (CLI → Daemon):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "click",
  "params": {
    "target": "3",
    "autoSnapshot": true
  }
}
```

**响应格式 (Daemon → CLI):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "ok": true,
    "action": "AXPress on button \"新建文件夹\"",
    "snapshot": {
      "app": "Finder",
      "window": "文稿",
      "elements": [
        {"ref": 1, "role": "button", "name": "后退", "enabled": true},
        {"ref": 2, "role": "button", "name": "前进", "enabled": false},
        {"ref": 3, "role": "textField", "name": "名称", "value": "", "focused": true}
      ]
    }
  }
}
```

### CLI 端实现逻辑 (TypeScript)

```typescript
// 伪代码，展示核心逻辑
async function main() {
  const args = parseArgs(process.argv);  // cu click 3 → {cmd: "click", target: "3"}

  // 1. 尝试连接 daemon
  let socket = await connectSocket(SOCKET_PATH);

  // 2. 如果 daemon 未运行，自动启动
  if (!socket) {
    await spawnDaemon();  // fork Swift 二进制到后台
    socket = await connectSocket(SOCKET_PATH, { retries: 5, delay: 100 });
  }

  // 3. 发送 JSON-RPC 请求
  const response = await sendRequest(socket, {
    method: args.cmd,
    params: args.params
  });

  // 4. 格式化输出
  if (process.stdout.isTTY) {
    printHuman(response);  // 彩色、表格、缩进
  } else {
    printJSON(response);   // 紧凑 JSON，供 agent 解析
  }

  // 5. 设置退出码
  process.exit(response.ok ? 0 : 1);
}
```

### Daemon 端实现逻辑 (Swift)

```swift
// 伪代码，展示核心结构
class Daemon {
    let socketServer: UnixSocketServer
    let axEngine: AXEngine
    let scriptEngine: ScriptEngine
    let eventEngine: CGEventEngine
    let visionEngine: VisionEngine
    let router: Router
    let stateManager: StateManager

    func handleRequest(_ request: JSONRPCRequest) -> JSONRPCResponse {
        // 路由到对应的引擎
        switch request.method {
        case "apps":
            return listApps()
        case "snapshot":
            return axEngine.snapshot(app: request.params.app)
        case "click":
            return router.click(target: request.params.target)
        case "type":
            return router.type(target: request.params.target, text: request.params.text)
        case "key":
            return eventEngine.pressKey(combo: request.params.combo)
        case "script":
            return scriptEngine.run(app: request.params.app, action: request.params.action)
        case "system":
            return systemCommand(request.params)
        case "screenshot":
            return visionEngine.captureScreen()
        // ... 更多命令
        }
    }
}
```

---

## 3. Layer 0: 系统 CLI/API 引擎

### 原理

Layer 0 是最简单也是最可靠的层：直接调用 macOS 内建的 CLI 工具。不涉及任何 UI 交互。

```
Agent: "把音量设到 50%"
  ↓
cu system volume 50
  ↓
Daemon 执行: Process("/usr/bin/osascript", ["-e", "set volume output volume 50"])
  ↓
返回: {"ok": true}
```

### 实现方式

Swift Daemon 通过 `Process` (即 `NSTask`) 执行 shell 命令：

```swift
func executeShell(command: String, args: [String]) -> (stdout: String, stderr: String, exitCode: Int32) {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: command)
    process.arguments = args

    let stdoutPipe = Pipe()
    let stderrPipe = Pipe()
    process.standardOutput = stdoutPipe
    process.standardError = stderrPipe

    try process.run()
    process.waitUntilExit()

    let stdout = String(data: stdoutPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
    let stderr = String(data: stderrPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""

    return (stdout, stderr, process.terminationStatus)
}
```

### 命令映射表

| `cu` 命令 | 底层调用 |
|-----------|---------|
| `cu system volume 50` | `osascript -e 'set volume output volume 50'` |
| `cu system volume mute` | `osascript -e 'set volume output muted true'` |
| `cu system wifi off` | `networksetup -setairportpower en0 off` |
| `cu system wifi connect "SSID"` | `networksetup -setairportnetwork en0 "SSID"` |
| `cu system dark-mode on` | `osascript -e 'tell app "System Events" to set dark mode of appearance preferences to true'` |
| `cu system brightness 70` | `brightness 0.7` (需 brew) 或 IOKit API |
| `cu system open "url"` | `/usr/bin/open "url"` |
| `cu system notify "title" "body"` | `osascript -e 'display notification "body" with title "title"'` |
| `cu file search "query"` | `mdfind "query"` |
| `cu file tag "/path" add "tag"` | `tag -a "tag" "/path"` 或 `xattr -w ...` |
| `cu file convert image "f" --to jpeg` | `sips -s format jpeg "f" --out "f.jpg"` |

### 为什么 Layer 0 重要？

以安装应用为例：

**GUI 方式 (Layer 4, 传统 agent 做法):**
1. 截图 → 发送给视觉模型 (2s)
2. 模型决定打开浏览器 → 截图 (2s)
3. 导航到下载页 → 截图 (2s)
4. 点击下载 → 等待下载 → 截图 (5s)
5. 打开 DMG → 截图 (2s)
6. 拖动应用到 Applications → 截图 (3s)
7. 弹出 DMG → 截图 (2s)
总计：~18s，7次 LLM 调用，~10,000 tokens

**Layer 0 方式:**
```bash
cu system shell "brew install --cask slack"
```
总计：<1s CLI 调用 + 下载时间，0次 LLM 调用，~50 tokens

**差距：100x+**

---

## 4. Layer 1: AppleScript/JXA 语义引擎

### 原理

AppleScript/JXA 直接与应用的**内部模型**对话，不经过 UI。应用暴露「脚本字典」(scripting dictionary)，定义了它支持的对象和操作。

```
传统 GUI 方式 (Layer 2-4):
  Agent → 截图 → "我看到 Mail 的 Compose 按钮" → 点击 → 填写 To → 填写 Subject → 填写 Body → 点击 Send
  8+ 步骤，每步可能失败

AppleScript 方式 (Layer 1):
  Agent → cu script "Mail" send --to "x@y.com" --subject "Hi" --body "Hello"
  1 步骤，确定性成功
```

### 底层机制

macOS 的 Apple Event 机制：

```
cu script "Mail" send --to "..." --subject "..." --body "..."
  ↓
Daemon 生成 AppleScript:
  tell application "Mail"
      set msg to make new outgoing message with properties ¬
          {subject:"Hi", content:"Hello", visible:true}
      tell msg
          make new to recipient at end of to recipients ¬
              with properties {address:"x@y.com"}
      end tell
      send msg
  end tell
  ↓
Daemon 通过 NSAppleScript 或 Process("osascript") 执行
  ↓
macOS 通过 Apple Event IPC 发送命令到 Mail.app 进程
  ↓
Mail.app 内部执行操作（创建消息、设置收件人、发送）
  ↓
返回结果
```

### 两种执行方式

**方式 A: osascript 子进程 (简单，推荐起步)**
```swift
func runAppleScript(_ script: String) -> String {
    let result = executeShell(
        command: "/usr/bin/osascript",
        args: ["-e", script]
    )
    return result.stdout
}

// JXA 同理，加 -l JavaScript 参数
func runJXA(_ script: String) -> String {
    let result = executeShell(
        command: "/usr/bin/osascript",
        args: ["-l", "JavaScript", "-e", script]
    )
    return result.stdout
}
```

**方式 B: NSAppleScript 内嵌执行 (更快，无进程开销)**
```swift
func runAppleScriptInProcess(_ source: String) -> String? {
    let script = NSAppleScript(source: source)
    var error: NSDictionary?
    let result = script?.executeAndReturnError(&error)
    if let error = error {
        return nil  // 处理错误
    }
    return result?.stringValue
}
```

方式 B 省去了 fork/exec 开销 (~50ms)，但两者都远快于 GUI 操作。

### JXA 的 ObjC Bridge：隐藏核武器

JXA 不仅仅是 AppleScript 的 JavaScript 版。它有一个 ObjC bridge，可以调用 **任何 Cocoa 框架**：

```javascript
// 通过 osascript -l JavaScript 执行
ObjC.import("AppKit");
ObjC.import("CoreLocation");

// 获取所有运行的应用
var apps = $.NSWorkspace.sharedWorkspace.runningApplications;
var names = [];
for (var i = 0; i < apps.count; i++) {
    names.push(apps.objectAtIndex(i).localizedName.js);
}
JSON.stringify(names);  // 输出 JSON
```

**这意味着什么？** Swift daemon 不需要覆盖所有可能的 API。对于罕见操作，可以动态生成 JXA 代码并执行。例如：

- 获取 GPS 定位 → JXA 调用 CoreLocation
- 蓝牙操作 → JXA 调用 IOBluetooth
- Night Shift 控制 → JXA 调用 CoreBrightness (private framework)

### 脚本字典发现

每个 scriptable app 都有 `.sdef` 文件定义它的字典：

```swift
// 检查应用是否有脚本字典
func hasScriptingDictionary(bundlePath: String) -> Bool {
    let sdefPath = bundlePath + "/Contents/Resources/*.sdef"
    // 或通过 OSALanguage 框架查询
    return FileManager.default.fileExists(atPath: sdefPath)
}
```

可以解析 `.sdef` (XML 格式) 得知应用支持哪些对象和操作。但实践中，为常用应用预建命令模板更可靠。

### 预建命令模板

```swift
enum AppCommand {
    // Mail
    case mailSend(to: String, subject: String, body: String)
    case mailGetUnread

    // Calendar
    case calendarCreateEvent(title: String, startDate: Date, endDate: Date)
    case calendarListEvents(date: Date)

    // Safari
    case safariGetURL
    case safariGetTitle
    case safariOpenURL(url: String)

    // Chrome
    case chromeExecJS(code: String)
    case chromeGetURL

    // Finder
    case finderCreateFolder(path: String)
    case finderTag(path: String, tag: String)
    case finderGetSelection

    // Notes, Reminders, Music, Messages, Contacts, etc.
}

func generateScript(_ command: AppCommand) -> String {
    switch command {
    case .mailSend(let to, let subject, let body):
        return """
        tell application "Mail"
            set msg to make new outgoing message with properties ¬
                {subject:"\(subject.escaped)", content:"\(body.escaped)", visible:true}
            tell msg
                make new to recipient at end of to recipients ¬
                    with properties {address:"\(to.escaped)"}
            end tell
            send msg
        end tell
        """
    case .safariGetURL:
        return """
        tell application "Safari"
            get URL of current tab of front window
        end tell
        """
    // ... 每个命令一个模板
    }
}
```

---

## 5. Layer 2: AX Tree 引擎（核心）

这是 computer-pilot 最复杂也最重要的部分。

### 什么是 AX Tree？

macOS 的 Accessibility (无障碍) 框架为**每个应用**维护一棵 UI 元素树。这棵树描述了应用的所有可交互元素：

```
Application "Finder"
  └─ Window "文稿"
       ├─ Toolbar
       │    ├─ Button "后退"
       │    ├─ Button "前进"
       │    └─ Button "搜索"
       ├─ SplitGroup
       │    ├─ ScrollArea (侧边栏)
       │    │    ├─ Row "收藏"
       │    │    ├─ Row "桌面"
       │    │    └─ Row "文稿"
       │    └─ ScrollArea (内容区)
       │         ├─ Cell "项目A" (文件夹)
       │         ├─ Cell "报告.pdf"
       │         └─ Cell "照片.jpg"
       └─ Toolbar (底部)
            └─ StaticText "3 个项目"
```

### 底层 API: AXUIElement

```swift
import ApplicationServices

// 1. 获取目标应用的 AX 根元素
let pid = getProcessID(appName: "Finder")  // 通过 NSRunningApplication 获取
let appElement = AXUIElementCreateApplication(pid)

// 2. 获取元素属性（跨进程 IPC 调用）
var value: CFTypeRef?
AXUIElementCopyAttributeValue(appElement, kAXWindowsAttribute as CFString, &value)
let windows = value as! [AXUIElement]

// 3. 遍历子元素
var children: CFTypeRef?
AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &children)
let childArray = children as! [AXUIElement]

// 4. 读取属性
var role: CFTypeRef?
AXUIElementCopyAttributeValue(element, kAXRoleAttribute as CFString, &role)
let roleString = role as! String  // "AXButton", "AXTextField", etc.

// 5. 批量读取属性（3-5x 更快！）
let attributes: [CFString] = [
    kAXRoleAttribute, kAXTitleAttribute, kAXValueAttribute,
    kAXDescriptionAttribute, kAXPositionAttribute, kAXSizeAttribute,
    kAXEnabledAttribute, kAXFocusedAttribute
]
var values: CFArray?
AXUIElementCopyMultipleAttributeValues(
    element,
    attributes as CFArray,
    .stopOnError,  // 或 0 继续
    &values
)
```

### 关键知识点

**1. IPC 机制**

每次 `AXUIElementCopyAttributeValue` 调用都是**跨进程 IPC**：

```
cu 进程 → Daemon 进程 → (Mach IPC) → 目标应用进程 → 返回属性值
```

这意味着：
- 每次调用有 ~1-5ms 延迟
- 如果目标应用卡死，调用会 hang（需要设置超时）
- 批量读取 (`CopyMultipleAttributeValues`) 可以将多个 IPC 合并为一个

**2. 超时控制**

```swift
// 全局 AX 超时设置
AXUIElementSetMessagingTimeout(element, 3.0)  // 3 秒超时

// 或通过 kAXDefaultTimeout 设置默认值（6 秒）
```

Chrome/Electron 应用特别容易 hang（AX tree 巨大且更新频繁），必须设超时。

**3. 树遍历策略**

```swift
func traverseAXTree(
    element: AXUIElement,
    depth: Int = 0,
    maxDepth: Int = 15,
    refs: inout [Int: AXElementInfo]
) {
    guard depth < maxDepth else { return }

    // 批量读取所有需要的属性
    let attrs = batchReadAttributes(element)

    // 只给可交互元素分配 ref 编号
    let role = attrs.role  // "AXButton", "AXTextField", "AXCheckBox", etc.
    if isInteractiveRole(role) {
        let refId = refs.count + 1
        refs[refId] = AXElementInfo(
            element: element,
            role: role,
            name: attrs.title ?? attrs.description ?? "",
            value: attrs.value,
            position: attrs.position,
            size: attrs.size,
            enabled: attrs.enabled ?? true,
            focused: attrs.focused ?? false
        )
    }

    // 递归遍历子元素
    // 关键优化：无名的 AXGroup 不消耗深度预算（agent-desktop 的做法）
    let children = getChildren(element)
    let nextDepth = (role == "AXGroup" && attrs.title == nil) ? depth : depth + 1
    for child in children {
        traverseAXTree(element: child, depth: nextDepth, maxDepth: maxDepth, refs: &refs)
    }
}
```

**4. 哪些角色是可交互的？**

agent-desktop 定义了 16 种可交互角色：

```swift
let interactiveRoles: Set<String> = [
    "AXButton",           // 按钮
    "AXCheckBox",         // 复选框
    "AXRadioButton",      // 单选按钮
    "AXTextField",        // 文本输入框
    "AXTextArea",         // 多行文本框
    "AXPopUpButton",      // 下拉菜单
    "AXComboBox",         // 组合框
    "AXSlider",           // 滑块
    "AXIncrementor",      // 步进器
    "AXMenuItem",         // 菜单项
    "AXLink",             // 链接
    "AXCell",             // 表格单元格
    "AXRow",              // 表格行
    "AXTab",              // 标签页
    "AXDisclosureTriangle", // 展开/折叠三角
    "AXColorWell"         // 颜色选择器
]
```

不可交互元素（AXStaticText、AXImage、AXGroup 等）不分配 ref，避免数字膨胀。

### 15 步点击链（Click Chain）

这是 agent-desktop 最精妙的设计。当 agent 调用 `cu click 3` 时，不是简单地发送鼠标事件，而是**按优先级尝试 15 种不同的激活方式**：

```swift
func performClick(element: AXUIElement) -> Bool {
    // Step 1: AXPress — 最标准的方式，对大多数按钮有效
    if tryAction(element, action: kAXPressAction) { return true }

    // Step 2: AXConfirm — 对话框确认按钮
    if tryAction(element, action: kAXConfirmAction) { return true }

    // Step 3: AXOpen — 列表项、文件项
    if tryAction(element, action: kAXOpenAction) { return true }

    // Step 4: AXPick — 弹出菜单选项
    if tryAction(element, action: kAXPickAction) { return true }

    // Step 5: AXShowAlternateUI — 显示替代 UI（如工具栏隐藏按钮）
    if tryAction(element, action: "AXShowAlternateUI") { return true }

    // Step 6: 尝试子元素的 AXPress/Confirm/Open
    if let children = getChildren(element) {
        for child in children {
            if tryAction(child, action: kAXPressAction) { return true }
            if tryAction(child, action: kAXConfirmAction) { return true }
            if tryAction(child, action: kAXOpenAction) { return true }
        }
    }

    // Step 7: 值传递 — 设置 AXValue
    if setValueIfApplicable(element) { return true }

    // Step 8: 设置 AXSelected = true
    if trySetAttribute(element, attr: kAXSelectedAttribute, value: true) { return true }

    // Step 9: 选择父行
    if let parentRow = findAncestor(element, role: "AXRow") {
        if trySetAttribute(parentRow, attr: kAXSelectedAttribute, value: true) { return true }
    }

    // Step 10: 选择父表格
    if let parentTable = findAncestor(element, role: "AXTable") {
        if trySetAttribute(parentTable, attr: kAXSelectedAttribute, value: true) { return true }
    }

    // Step 11: 自定义 actions
    if let actions = getAvailableActions(element) {
        for action in actions where !standardActions.contains(action) {
            if tryAction(element, action: action) { return true }
        }
    }

    // Step 12: 先聚焦，再 Confirm/Press
    if trySetAttribute(element, attr: kAXFocusedAttribute, value: true) {
        if tryAction(element, action: kAXConfirmAction) { return true }
        if tryAction(element, action: kAXPressAction) { return true }
    }

    // Step 13: 聚焦 + 键盘空格键（模拟点击）
    if trySetAttribute(element, attr: kAXFocusedAttribute, value: true) {
        pressKey(.space)  // CGEvent
        return true
    }

    // Step 14: 尝试祖先元素的 Press/Confirm
    if let parent = getParent(element) {
        if tryAction(parent, action: kAXPressAction) { return true }
        if tryAction(parent, action: kAXConfirmAction) { return true }
    }

    // Step 15: 终极兜底 — CGEvent 鼠标点击（Layer 3）
    let position = getElementCenter(element)
    return performCGEventClick(at: position)
}
```

**为什么需要这么多步骤？**

不同的 UI 控件对不同的 AX action 响应：
- 普通 Button → AXPress 就够
- 列表项 → AXOpen 有效，AXPress 可能无效
- 下拉菜单选项 → AXPick
- 表格行 → AXSelected = true
- 某些自定义控件 → 只响应 CGEvent 鼠标事件

15 步链保证了**几乎任何控件都能被成功激活**，同时优先使用最可靠的方式。

### Snapshot 输出格式

```swift
func formatSnapshot(refs: [Int: AXElementInfo]) -> String {
    // 对 agent 友好的紧凑格式
    var lines: [String] = []
    for (id, info) in refs.sorted(by: { $0.key < $1.key }) {
        var parts = ["[\(id)]", info.role.replacingOccurrences(of: "AX", with: "").lowercased()]

        if let name = info.name, !name.isEmpty {
            parts.append("\"\(name)\"")
        }
        if let value = info.value, !value.isEmpty {
            parts.append("value=\"\(value)\"")
        }
        if !info.enabled {
            parts.append("(disabled)")
        }
        if info.focused {
            parts.append("(focused)")
        }

        lines.append(parts.joined(separator: " "))
    }
    return lines.joined(separator: "\n")
}
```

输出示例：
```
[1] button "后退"
[2] button "前进" (disabled)
[3] button "搜索"
[4] textfield "搜索" (focused)
[5] row "桌面"
[6] row "文稿"
[7] cell "项目A"
[8] cell "报告.pdf"
[9] cell "照片.jpg"
```

每行 ~30 字符，50 个元素 = ~1500 字符 ≈ ~500 tokens。远低于截图的 ~1400 tokens，且信息更可操作。

---

## 6. Layer 3: CGEvent 输入合成引擎

### 原理

CGEvent 是 macOS 的低级输入事件系统。它在 Window Server 层注入事件，对所有应用生效。

### 鼠标操作

```swift
import CoreGraphics

// 鼠标移动
func moveMouse(to point: CGPoint) {
    let event = CGEvent(mouseEventSource: nil,
                        mouseType: .mouseMoved,
                        mouseCursorPosition: point,
                        mouseButton: .left)
    event?.post(tap: .cghidEventTap)
}

// 鼠标点击
func click(at point: CGPoint, button: CGMouseButton = .left) {
    // 1. 移动到目标位置
    let moveEvent = CGEvent(mouseEventSource: nil,
                            mouseType: .mouseMoved,
                            mouseCursorPosition: point,
                            mouseButton: button)
    moveEvent?.post(tap: .cghidEventTap)

    // 2. 发送 mouseDown
    let downType: CGEventType = button == .left ? .leftMouseDown : .rightMouseDown
    let downEvent = CGEvent(mouseEventSource: nil,
                            mouseType: downType,
                            mouseCursorPosition: point,
                            mouseButton: button)
    downEvent?.post(tap: .cghidEventTap)

    // 3. 短暂延迟（模拟人类点击）
    usleep(50_000)  // 50ms

    // 4. 发送 mouseUp
    let upType: CGEventType = button == .left ? .leftMouseUp : .rightMouseUp
    let upEvent = CGEvent(mouseEventSource: nil,
                          mouseType: upType,
                          mouseCursorPosition: point,
                          mouseButton: button)
    upEvent?.post(tap: .cghidEventTap)
}

// 双击
func doubleClick(at point: CGPoint) {
    let downEvent = CGEvent(mouseEventSource: nil,
                            mouseType: .leftMouseDown,
                            mouseCursorPosition: point,
                            mouseButton: .left)
    downEvent?.setIntegerValueField(.mouseEventClickState, value: 2)  // 关键！
    downEvent?.post(tap: .cghidEventTap)

    let upEvent = CGEvent(mouseEventSource: nil,
                          mouseType: .leftMouseUp,
                          mouseCursorPosition: point,
                          mouseButton: .left)
    upEvent?.setIntegerValueField(.mouseEventClickState, value: 2)
    upEvent?.post(tap: .cghidEventTap)
}

// 拖拽
func drag(from start: CGPoint, to end: CGPoint) {
    // mouseDown at start
    let downEvent = CGEvent(mouseEventSource: nil,
                            mouseType: .leftMouseDown,
                            mouseCursorPosition: start,
                            mouseButton: .left)
    downEvent?.post(tap: .cghidEventTap)

    // 分步移动到终点（不能瞬移，否则某些应用不识别）
    let steps = 20
    for i in 1...steps {
        let t = CGFloat(i) / CGFloat(steps)
        let point = CGPoint(
            x: start.x + (end.x - start.x) * t,
            y: start.y + (end.y - start.y) * t
        )
        let dragEvent = CGEvent(mouseEventSource: nil,
                                mouseType: .leftMouseDragged,
                                mouseCursorPosition: point,
                                mouseButton: .left)
        dragEvent?.post(tap: .cghidEventTap)
        usleep(10_000)  // 10ms between steps
    }

    // mouseUp at end
    let upEvent = CGEvent(mouseEventSource: nil,
                          mouseType: .leftMouseUp,
                          mouseCursorPosition: end,
                          mouseButton: .left)
    upEvent?.post(tap: .cghidEventTap)
}

// 滚动
func scroll(deltaY: Int, deltaX: Int = 0) {
    let event = CGEvent(scrollWheelEvent2Source: nil,
                        units: .pixel,
                        wheelCount: 2,
                        wheel1: Int32(deltaY),
                        wheel2: Int32(deltaX))
    event?.post(tap: .cghidEventTap)
}
```

### 键盘操作

```swift
// 按键
func pressKey(keyCode: CGKeyCode, flags: CGEventFlags = []) {
    let source = CGEventSource(stateID: .combinedSessionState)

    // keyDown
    let keyDown = CGEvent(keyboardEventSource: source,
                          virtualKey: keyCode,
                          keyDown: true)
    keyDown?.flags = flags
    keyDown?.post(tap: .cghidEventTap)

    // keyUp
    let keyUp = CGEvent(keyboardEventSource: source,
                        virtualKey: keyCode,
                        keyDown: false)
    keyUp?.flags = flags
    keyUp?.post(tap: .cghidEventTap)
}

// 组合键 (如 cmd+c)
func pressCombo(combo: String) {
    // 解析 "cmd+shift+s" → flags + keyCode
    let parts = combo.lowercased().split(separator: "+")
    var flags: CGEventFlags = []
    var key: String = ""

    for part in parts {
        switch part {
        case "cmd", "command": flags.insert(.maskCommand)
        case "shift": flags.insert(.maskShift)
        case "alt", "option": flags.insert(.maskAlternate)
        case "ctrl", "control": flags.insert(.maskControl)
        default: key = String(part)
        }
    }

    let keyCode = keyCodeMap[key] ?? 0  // 查表：a=0, s=1, d=2, ...
    pressKey(keyCode: keyCode, flags: flags)
}

// 打字（输入文本）
func typeText(_ text: String) {
    let source = CGEventSource(stateID: .combinedSessionState)
    let event = CGEvent(keyboardEventSource: source,
                        virtualKey: 0,
                        keyDown: true)

    // 使用 keyboardSetUnicodeString 支持任意 Unicode
    let chars = Array(text.utf16)
    // 每次最多 20 个字符
    for chunk in stride(from: 0, to: chars.count, by: 20) {
        let end = min(chunk + 20, chars.count)
        let slice = Array(chars[chunk..<end])

        event?.keyboardSetUnicodeString(stringLength: slice.count,
                                        unicodeString: slice)
        event?.post(tap: .cghidEventTap)

        // keyUp 也要发
        let upEvent = CGEvent(keyboardEventSource: source,
                              virtualKey: 0,
                              keyDown: false)
        upEvent?.post(tap: .cghidEventTap)
    }
}
```

### Retina / HiDPI 坐标换算

```
物理像素 ≠ 逻辑点

Retina 显示器 (2x)：
  物理像素: 2880 x 1800
  逻辑点:   1440 x 900   ← CGEvent 使用这个坐标系

AX API 返回的 AXPosition/AXSize 已经是逻辑点，直接传给 CGEvent 即可。
但如果从截图提取坐标，需要换算。
```

```swift
func imageCoordToScreenCoord(imageX: Int, imageY: Int,
                              imageWidth: Int, imageHeight: Int,
                              screenWidth: Int, screenHeight: Int) -> CGPoint {
    let scaleX = CGFloat(screenWidth) / CGFloat(imageWidth)
    let scaleY = CGFloat(screenHeight) / CGFloat(imageHeight)
    return CGPoint(x: CGFloat(imageX) * scaleX,
                   y: CGFloat(imageY) * scaleY)
}
```

---

## 7. Layer 4: 视觉引擎

### 截图：ScreenCaptureKit

```swift
import ScreenCaptureKit

func captureScreen() async throws -> CGImage {
    // 1. 获取可用显示器和窗口
    let content = try await SCShareableContent.current

    // 2. 选择主显示器
    guard let display = content.displays.first else { throw CaptureError.noDisplay }

    // 3. 配置截图参数
    let filter = SCContentFilter(display: display, excludingWindows: [])
    let config = SCStreamConfiguration()
    config.width = display.width
    config.height = display.height
    config.pixelFormat = kCVPixelFormatType_32BGRA
    config.showsCursor = true

    // 4. 捕获截图（硬件加速，5-15ms）
    let image = try await SCScreenshotManager.captureImage(
        contentFilter: filter,
        configuration: config
    )
    return image
}

// 也可以按窗口截图（不需要窗口在前台！）
func captureWindow(windowID: CGWindowID) async throws -> CGImage {
    let content = try await SCShareableContent.current
    let window = content.windows.first { $0.windowID == windowID }
    let filter = SCContentFilter(desktopIndependentWindow: window!)
    // ... 同上
}
```

### OCR：Vision Framework

```swift
import Vision

func performOCR(on image: CGImage) throws -> [TextObservation] {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate  // .fast 更快但精度低
    request.recognitionLanguages = ["zh-Hans", "en-US"]  // 中英文
    request.usesLanguageCorrection = true

    let handler = VNImageRequestHandler(cgImage: image)
    try handler.perform([request])

    guard let results = request.results else { return [] }

    return results.map { observation in
        let text = observation.topCandidates(1).first?.string ?? ""
        let boundingBox = observation.boundingBox  // 归一化坐标 (0-1)

        // 转换为屏幕坐标
        let screenX = boundingBox.origin.x * CGFloat(image.width)
        let screenY = (1 - boundingBox.origin.y - boundingBox.height) * CGFloat(image.height)
        let width = boundingBox.width * CGFloat(image.width)
        let height = boundingBox.height * CGFloat(image.height)

        return TextObservation(
            text: text,
            x: Int(screenX),
            y: Int(screenY),
            width: Int(width),
            height: Int(height),
            confidence: observation.topCandidates(1).first?.confidence ?? 0
        )
    }
}
```

### 为什么 Vision OCR 比截图发给 LLM 好？

```
方案 A: 截图 → 编码为 base64 PNG → 发给 LLM 视觉模型
  延迟: 2-10s (网络 + 推理)
  成本: ~1,400 tokens (图片) + ~500 tokens (响应)
  结果: LLM 返回文字描述 + 坐标猜测

方案 B: 截图 → 本地 Vision OCR → 文字 + 精确坐标
  延迟: 200-500ms (本地计算)
  成本: ~500 tokens (OCR 文字结果，无图片 tokens)
  结果: 精确的文字内容 + 精确的边界框坐标
```

方案 B 快 10x，便宜 3-4x，且坐标更准确。只有当 OCR 无法识别（如图标、复杂图形界面）时，才需要方案 A。

---

## 8. 智能路由器

### 路由决策流程

当 agent 调用 `cu click 3` 时，路由器执行以下决策：

```swift
func routeClick(target: String) -> ActionResult {
    // 1. 解析 target
    if let ref = Int(target) {
        // Ref-based click
        return routeRefClick(ref: ref)
    } else if target.contains(",") {
        // Coordinate-based click: "500,300"
        let parts = target.split(separator: ",")
        let point = CGPoint(x: Double(parts[0])!, y: Double(parts[1])!)
        return eventEngine.click(at: point)  // 直接 Layer 3
    } else {
        return ActionResult(ok: false, error: "Invalid target: \(target)",
                           hint: "Use ref number (cu click 3) or coordinates (cu click 500,300)")
    }
}

func routeRefClick(ref: Int) -> ActionResult {
    // 1. 查找 ref
    guard let elementInfo = stateManager.refs[ref] else {
        return ActionResult(ok: false,
                           error: "Ref [\(ref)] not found. Snapshot has \(stateManager.refs.count) elements.",
                           hint: "Run 'cu snapshot \"\(stateManager.currentApp)\"' to refresh.")
    }

    // 2. 尝试 Layer 1: AppleScript 语义操作（如果适用）
    if let scriptAction = findScriptableAction(element: elementInfo) {
        let result = scriptEngine.run(scriptAction)
        if result.ok { return autoSnapshot(result) }
    }

    // 3. 尝试 Layer 2: AX 15-step click chain
    let axResult = axEngine.performClick(element: elementInfo.element)
    if axResult {
        return autoSnapshot(ActionResult(ok: true, action: "AX click on \(elementInfo.description)"))
    }

    // 4. 兜底 Layer 3: CGEvent mouse click
    let center = getElementCenter(elementInfo)
    eventEngine.click(at: center)
    return autoSnapshot(ActionResult(ok: true, action: "CGEvent click at \(center)"))
}
```

### 路由器如何决定用 AppleScript 还是 AX？

```swift
func findScriptableAction(element: AXElementInfo) -> AppCommand? {
    let app = stateManager.currentApp

    // 检查是否是 scriptable app 的已知语义操作
    // 例如：在 Mail 中点击 "发送" 按钮 → 直接用 AppleScript send
    if app == "Mail" && element.name == "发送" && element.role == "AXButton" {
        // 但需要上下文 — 当前正在编辑的消息
        // 实际实现中，这需要更复杂的上下文分析
        return nil  // 暂时回退到 AX
    }

    // 在 Safari 中点击地址栏 → 用 AppleScript 获取/设置 URL
    if app == "Safari" && element.role == "AXTextField" && element.name?.contains("地址") == true {
        // 这种情况下 AX type 更合适
        return nil
    }

    // 通常，对于 UI 元素点击，AX 是更好的选择
    // AppleScript 适合语义操作（send, create, delete），不适合 UI 元素点击
    return nil
}
```

**实践规则：** AppleScript 适合 `cu script` 命令（语义操作），AX 适合 `cu click/type` 命令（UI 交互）。路由器在大多数 click/type 场景下直接使用 AX chain。

---

## 9. Ref 系统

### 分配规则

```
Ref 分配 = DFS 遍历顺序 × 仅可交互元素
```

```swift
// DFS 遍历时，按遇到的顺序给可交互元素编号
var nextRef = 1

func assignRefs(element: AXUIElement, depth: Int) {
    let attrs = batchReadAttributes(element)

    if isInteractiveRole(attrs.role) {
        refMap[nextRef] = AXElementInfo(element: element, ...)
        nextRef += 1
    }

    for child in getChildren(element) {
        assignRefs(element: child, depth: depth + 1)
    }
}
```

DFS 顺序的好处：ref 编号大致对应**从上到下、从左到右**的视觉布局，agent 能直觉地理解 [1] 在 [5] 上面。

### Ref 刷新策略

**每次动作后刷新 ref**（不尝试维护稳定 ref）。

为什么？
- UI 在每次点击后可能完全改变（新窗口、对话框、页面跳转）
- 维护稳定 ref 需要复杂的元素匹配算法，且在 UI 大变化时必然失败
- Agent 每次拿到新 snapshot 就知道当前所有可用 ref
- browser-pilot 的经验证明：**刷新 ref 比维护 ref 更可靠**

### Ref 解析

```swift
// 点击时，通过 ref 找到元素
func resolveRef(_ ref: Int) -> AXUIElement? {
    guard let info = refMap[ref] else { return nil }

    // 验证元素是否仍然有效
    // AXUIElement 可能已经失效（元素被销毁）
    var role: CFTypeRef?
    let status = AXUIElementCopyAttributeValue(info.element, kAXRoleAttribute as CFString, &role)

    if status == .success {
        return info.element  // 仍然有效
    } else {
        // 元素已失效，需要重新 snapshot
        return nil
    }
}
```

---

## 10. Daemon 状态管理

### 维护的状态

```swift
class StateManager {
    // 当前焦点应用
    var currentApp: String = ""

    // 当前应用的 PID
    var currentPID: pid_t = 0

    // 当前 ref 映射
    var refMap: [Int: AXElementInfo] = [:]

    // 已知 scriptable apps（启动时检测一次）
    var scriptableApps: Set<String> = []

    // 权限状态
    var permissions: PermissionStatus = PermissionStatus()

    // AX tree 缓存（可选优化）
    var axTreeCache: [pid_t: CachedAXTree] = [:]
    var axTreeCacheTimestamp: [pid_t: Date] = [:]
    let axTreeCacheTTL: TimeInterval = 2.0  // 2 秒过期

    // 安全配置
    var safetyConfig: SafetyConfig = SafetyConfig.default
}
```

### Daemon 生命周期

```swift
class Daemon {
    func start() {
        // 1. 检查权限
        checkPermissions()

        // 2. 检测 scriptable apps
        detectScriptableApps()

        // 3. 创建 Unix socket 监听
        let socketPath = "/tmp/computer-pilot-\(getuid()).sock"
        // 如果已存在旧 socket，删除
        unlink(socketPath)
        let server = UnixSocketServer(path: socketPath)

        // 4. 开始监听
        server.onConnection { client in
            self.handleClient(client)
        }

        // 5. 设置空闲超时（30 分钟无活动自动退出）
        startIdleTimer(timeout: 30 * 60)

        // 6. 注册信号处理器（优雅退出）
        signal(SIGTERM) { _ in cleanup() }
        signal(SIGINT) { _ in cleanup() }

        RunLoop.main.run()
    }

    func handleClient(_ client: SocketConnection) {
        resetIdleTimer()

        while let data = client.readLine() {
            let request = try JSONDecoder().decode(JSONRPCRequest.self, from: data)
            let response = handleRequest(request)
            let responseData = try JSONEncoder().encode(response)
            client.write(responseData)
        }
    }
}
```

### 自动启动

TypeScript CLI 端：

```typescript
async function ensureDaemon(): Promise<Socket> {
    const socketPath = `/tmp/computer-pilot-${process.getuid()}.sock`;

    // 尝试连接
    try {
        return await connectSocket(socketPath);
    } catch {
        // Daemon 未运行，启动它
        const helperPath = path.join(__dirname, '../bin/desktop-helper');
        const child = spawn(helperPath, ['--daemon'], {
            detached: true,        // 脱离父进程
            stdio: 'ignore',       // 不继承 stdio
        });
        child.unref();             // 不等待子进程

        // 等待 socket 就绪
        for (let i = 0; i < 50; i++) {  // 最多等 5 秒
            await sleep(100);
            try {
                return await connectSocket(socketPath);
            } catch {
                continue;
            }
        }
        throw new Error('Failed to start daemon');
    }
}
```

---

## 11. Auto-Snapshot

### 原理

browser-pilot 最重要的设计决策之一：**每次动作后自动返回更新后的状态**。

```
没有 auto-snapshot:
  Agent: cu click 3       → {"ok": true}
  Agent: cu snapshot "App" → [1] button "X"  [2] textfield "Name" ...
  (两次调用，双倍 latency)

有 auto-snapshot:
  Agent: cu click 3       → {"ok": true, "snapshot": {"elements": [...]}}
  (一次调用，同时拿到结果和新状态)
```

### 实现

```swift
func autoSnapshot(_ result: ActionResult) -> ActionResult {
    // 等待 UI 更新
    usleep(200_000)  // 200ms — 给 UI 时间响应

    // 重新获取 AX tree
    let newRefs = axEngine.snapshot(pid: stateManager.currentPID)
    stateManager.refMap = newRefs

    // 附加到结果
    var enrichedResult = result
    enrichedResult.snapshot = formatSnapshot(newRefs)
    return enrichedResult
}
```

### 200ms 延迟的考量

为什么等 200ms？
- UI 更新不是即时的。点击按钮后，新窗口/对话框可能需要 50-200ms 出现
- 太短 (50ms)：新 UI 还没渲染，snapshot 是旧状态
- 太长 (500ms)：浪费时间
- 200ms 是经验值，覆盖 95%+ 的场景
- 如果 agent 发现 snapshot 没变化，可以调用 `cu wait --changed` 等待

---

## 12. 安全引擎

### 动作分类

```swift
enum ActionSafety {
    case safe           // 只读操作，无需确认
    case normal         // 普通交互，无需确认
    case caution        // 可能有副作用，记录日志
    case destructive    // 破坏性操作，需要确认
    case blocked        // 禁止执行
}

func classifyAction(command: String, params: [String: Any]) -> ActionSafety {
    switch command {
    // 安全：只读操作
    case "apps", "snapshot", "screenshot", "ocr", "permissions", "window list":
        return .safe

    // 正常：普通交互
    case "click":
        // 检查元素名是否包含危险关键词
        if let ref = params["target"] as? Int,
           let info = stateManager.refMap[ref] {
            let name = (info.name ?? "").lowercased()
            let destructiveKeywords = ["delete", "remove", "erase", "quit", "close all",
                                       "删除", "移除", "清除", "退出"]
            if destructiveKeywords.contains(where: { name.contains($0) }) {
                return .destructive
            }
        }
        return .normal

    case "type":
        // 检查是否在密码字段中输入
        if let ref = params["target"] as? Int,
           let info = stateManager.refMap[ref] {
            if info.role == "AXSecureTextField" {
                return .caution  // 密码字段，记录但不记录内容
            }
        }
        return .normal

    case "key":
        let combo = (params["combo"] as? String ?? "").lowercased()
        // 退出应用
        if combo == "cmd+q" { return .destructive }
        // 关闭窗口
        if combo == "cmd+w" { return .caution }
        return .normal

    case "system":
        let action = params["action"] as? String ?? ""
        // 关机/重启
        if ["shutdown", "restart", "sleep"].contains(action) { return .destructive }
        return .normal

    case "script raw":
        return .caution  // 原始 AppleScript 总是需要谨慎

    default:
        return .normal
    }
}
```

### 审计日志

```swift
func logAction(command: String, params: [String: Any], result: ActionResult, safety: ActionSafety) {
    let entry: [String: Any] = [
        "ts": ISO8601DateFormatter().string(from: Date()),
        "cmd": command,
        "params": sanitizeParams(params, safety: safety),  // 密码字段不记录值
        "app": stateManager.currentApp,
        "safety": String(describing: safety),
        "ok": result.ok,
        "action": result.action ?? "",
        "error": result.error ?? ""
    ]

    let line = try! JSONSerialization.data(withJSONObject: entry)
    auditFile.write(line)
    auditFile.write("\n".data(using: .utf8)!)
}
```

---

## 13. 自学习系统

### Recipe 录制原理

macOS 的 CGEvent Tap 可以**拦截所有输入事件**：

```swift
func startRecording() {
    // 创建 event tap（拦截所有键盘+鼠标事件）
    let eventMask: CGEventMask =
        (1 << CGEventType.keyDown.rawValue) |
        (1 << CGEventType.leftMouseDown.rawValue) |
        (1 << CGEventType.leftMouseUp.rawValue) |
        (1 << CGEventType.rightMouseDown.rawValue) |
        (1 << CGEventType.scrollWheel.rawValue)

    let tap = CGEvent.tapCreate(
        tap: .cgSessionEventTap,
        place: .tailAppendEventTap,  // 不拦截，只监听
        options: .listenOnly,         // 关键：只监听，不修改
        eventsOfInterest: eventMask,
        callback: { proxy, type, event, refcon -> Unmanaged<CGEvent>? in
            let recorder = Unmanaged<Recorder>.fromOpaque(refcon!).takeUnretainedValue()
            recorder.recordEvent(type: type, event: event)
            return Unmanaged.passRetained(event)
        },
        userInfo: Unmanaged.passUnretained(self).toOpaque()
    )

    // 需要 Input Monitoring 权限
    guard let tap = tap else {
        print("Error: Input Monitoring permission required")
        return
    }

    let runLoopSource = CFMachPortCreateRunLoopSource(nil, tap, 0)
    CFRunLoopAddSource(CFRunLoopGetCurrent(), runLoopSource, .commonModes)
    CGEvent.tapEnable(tap: tap, enable: true)
}
```

### 事件增强

每个原始事件被增强为语义事件：

```swift
func recordEvent(type: CGEventType, event: CGEvent) {
    let timestamp = Date()

    switch type {
    case .leftMouseDown:
        let point = event.location

        // 增强：查找点击位置的 AX 元素
        let axElement = findElementAtPoint(point)
        let appName = frontmostAppName()

        recordedEvents.append(RecordedEvent(
            timestamp: timestamp,
            type: .click,
            raw: RawClickEvent(x: point.x, y: point.y),
            enriched: EnrichedClickEvent(
                app: appName,
                elementRole: axElement?.role,
                elementName: axElement?.name,
                elementRef: nil  // 回放时重新查找
            )
        ))

    case .keyDown:
        let keyCode = event.getIntegerValueField(.keyboardEventKeycode)
        let chars = event.keyboardStringValue

        recordedEvents.append(RecordedEvent(
            timestamp: timestamp,
            type: .keyPress,
            raw: RawKeyEvent(keyCode: keyCode, chars: chars, flags: event.flags),
            enriched: EnrichedKeyEvent(
                combo: flagsToComboString(event.flags, keyCode: keyCode),
                text: chars
            )
        ))

    // ... scroll, drag, etc.
    }
}
```

### Recipe 合成

录制结束后，将原始事件序列发送给 LLM 合成为参数化 recipe：

```json
{
  "name": "rename-files",
  "description": "批量重命名文件，添加前缀",
  "parameters": {
    "folder": { "type": "string", "description": "目标文件夹路径" },
    "prefix": { "type": "string", "description": "文件名前缀" }
  },
  "steps": [
    {
      "action": "focus",
      "app": "Finder"
    },
    {
      "action": "script",
      "script": "tell application \"Finder\" to set target of front window to POSIX file \"{{folder}}\""
    },
    {
      "action": "key",
      "combo": "cmd+a",
      "description": "全选文件"
    },
    {
      "action": "key",
      "combo": "return",
      "description": "进入重命名模式"
    },
    {
      "action": "type",
      "text": "{{prefix}}",
      "description": "输入前缀"
    },
    {
      "action": "key",
      "combo": "return",
      "description": "确认重命名"
    }
  ]
}
```

### Recipe 回放

```swift
func runRecipe(name: String, params: [String: String]) throws {
    let recipe = loadRecipe(name)

    for step in recipe.steps {
        // 替换参数占位符
        let resolvedStep = resolveParams(step, params: params)

        switch resolvedStep.action {
        case "focus":
            activateApp(resolvedStep.app)
        case "script":
            runAppleScript(resolvedStep.script)
        case "click":
            if let ref = resolvedStep.ref {
                routeRefClick(ref: ref)
            } else if let name = resolvedStep.elementName {
                // 按名称查找元素并点击
                let ref = findRefByName(name)
                routeRefClick(ref: ref)
            }
        case "type":
            typeText(resolvedStep.text)
        case "key":
            pressCombo(resolvedStep.combo)
        case "wait":
            waitForCondition(resolvedStep.condition)
        }

        // 步间延迟
        usleep(UInt32(resolvedStep.delay ?? 200) * 1000)
    }
}
```

---

## 14. MCP Server 实现

### 原理

MCP Server 是一个**薄封装层**，将 CLI 命令暴露为 MCP 工具定义：

```
Agent (Claude Code / Codex)
  ↓ MCP JSON-RPC (stdio 或 HTTP)
MCP Server (TypeScript)
  ↓ Unix socket
Daemon (Swift)
  ↓ macOS APIs
操作系统
```

### 实现

```typescript
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";

const server = new Server({
    name: "computer-pilot",
    version: "1.0.0"
}, {
    capabilities: {
        tools: {}
    }
});

// 注册工具
server.setRequestHandler("tools/list", async () => ({
    tools: [
        {
            name: "cu_apps",
            description: "List running applications",
            inputSchema: { type: "object", properties: {} }
        },
        {
            name: "cu_snapshot",
            description: "Get AX tree snapshot with numbered refs for an app",
            inputSchema: {
                type: "object",
                properties: {
                    app: { type: "string", description: "App name" }
                },
                required: ["app"]
            }
        },
        {
            name: "cu_click",
            description: "Click element by ref number or coordinates. Returns updated snapshot.",
            inputSchema: {
                type: "object",
                properties: {
                    target: {
                        type: "string",
                        description: "Ref number (e.g. '3') or coordinates (e.g. '500,300')"
                    }
                },
                required: ["target"]
            }
        },
        // ... 其他工具
    ]
}));

// 处理工具调用
server.setRequestHandler("tools/call", async (request) => {
    const { name, arguments: args } = request.params;

    // 将 MCP 调用转换为 daemon 请求
    const method = name.replace("cu_", "");
    const response = await sendToDaemon({ method, params: args });

    return {
        content: [{
            type: "text",
            text: JSON.stringify(response, null, 2)
        }]
    };
});

// 启动 stdio 传输
const transport = new StdioServerTransport();
await server.connect(transport);
```

### 启动方式

```bash
# 方式 1: 作为 Claude Code MCP server
# 在 .claude.json 中配置:
# { "mcpServers": { "computer-pilot": { "command": "cu", "args": ["mcp", "serve"] } } }

# 方式 2: 直接运行
cu mcp serve              # stdio 模式
cu mcp serve --http 3000  # HTTP 模式（多客户端）
```

### 关键设计：MCP 是 CLI 的封装，不是替代

```
cu click 3                    ← Agent 通过 Bash 调用
cu_click(target: "3")         ← Agent 通过 MCP 调用

两者内部都执行完全相同的代码路径：
  → daemon.routeClick(target: "3")
  → axEngine.performClick(ref: 3)
  → autoSnapshot()
  → return result
```

---

## 总结：数据流全景

```
Agent 说: "点击 Finder 中的'新建文件夹'按钮"
  ↓
Agent 调用: cu click 3  (或 MCP cu_click target=3)
  ↓
TypeScript CLI:
  1. 解析参数: cmd="click", target="3"
  2. 连接 daemon (Unix socket, <10ms)
  3. 发送 JSON-RPC: {"method":"click","params":{"target":"3"}}
  ↓
Swift Daemon:
  4. 安全检查: classifyAction("click", ref=3) → .normal
  5. 路由决策:
     a. 解析 ref 3 → AXElementInfo{role: "AXButton", name: "新建文件夹", app: "Finder"}
     b. 是否有 AppleScript 语义操作？→ No (UI 按钮不适合)
     c. 尝试 AX click chain:
        - Step 1: AXPress → 成功！
  6. 记审计日志: {"cmd":"click","ref":3,"app":"Finder","action":"AXPress","ok":true}
  7. Auto-snapshot:
     a. 等待 200ms (UI 更新)
     b. 重新遍历 AX tree
     c. 分配新 ref (新对话框出现了)
  8. 返回 JSON-RPC 响应
  ↓
TypeScript CLI:
  9. 格式化输出:
     TTY: 彩色表格
     Pipe: 紧凑 JSON
  10. exit(0)
  ↓
Agent 收到:
  {"ok":true, "action":"AXPress on button \"新建文件夹\"",
   "snapshot":{"app":"Finder","elements":[
     {"ref":1,"role":"textfield","name":"名称","value":"","focused":true},
     {"ref":2,"role":"button","name":"取消"},
     {"ref":3,"role":"button","name":"创建"}
   ]}}
  ↓
Agent 知道: 出现了新建文件夹对话框，[1] 是名称输入框（已聚焦），
           接下来应该 cu type 1 "项目资料"

总延迟: <500ms (10ms IPC + 5ms AXPress + 200ms wait + 200ms re-snapshot)
总 token: ~200 (JSON 响应)
```
