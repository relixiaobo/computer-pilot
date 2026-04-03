# Computer Pilot — Competitive Analysis

> 每完成一项能力就更新此表。✅ = 已实现，❌ = 未实现，⭕ = 本轮新增。

## 感知（Perception）

| 能力 | cu (ours) | Anthropic Computer Use | Ghost OS | agent-desktop | usecomputer | axcli |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| AX Tree 快照 | ✅ | ❌ | ✅ | ✅ | ❌ | ✅ |
| 批量 AX 属性读取 | ⭕ | — | ✅ | ✅ | — | ✅ |
| Per-element 超时 | ❌ | — | ✅ 3s | ✅ 2s | — | ✅ |
| Numbered refs | ✅ | ❌ | ❌ | ✅ | ❌ | ❌ |
| Ref 持久化 | ❌ | — | ❌ | ✅ | — | — |
| 截图 | ✅ CGWindowList | ✅ | ✅ | ❌ | ✅ | ✅ |
| 无需激活截图 | ✅ | ✅ | ✅ | — | ✅ | ✅ |
| OCR | ⭕ Vision (objc2) | ❌ | ❌ | ❌ | ❌ | ✅ |
| 视觉模型 fallback | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ |
| Chrome CDP | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| 坐标偏移量 | ✅ | — | ❌ | ❌ | ✅ | ❌ |
| Auto-snapshot | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |

## 操作（Action）

| 能力 | cu (ours) | Anthropic Computer Use | Ghost OS | agent-desktop | usecomputer | axcli |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| 左键点击 | ✅ AX优先 | ✅ | ✅ | ✅ 15步 | ✅ | ✅ |
| 右键点击 | ✅ | ✅ | ❓ | ✅ | ✅ | ❌ |
| 双击 | ⭕ | ✅ | ❓ | ✅ | ✅ | ✅ |
| AX 点击链 | ⭕ 14步 | — | 基础 | **15步** | — | 基础 |
| 键盘快捷键 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 布局安全 | ✅ --app | ✅ | ✅ | ❓ | ✅ | ❓ |
| 文字输入 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 滚动 | ⭕ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 拖拽 | ⭕ | ✅ | ❓ | ✅ | ✅ | ❌ |
| Hover | ⭕ | ✅ | ❓ | ✅ | ✅ | ✅ |
| Hold key / 修饰键 | ⭕ --shift/--cmd/--alt | ✅ | ❌ | ❌ | ✅ | ❌ |
| Wait 条件 | ⭕ --text/--ref/--gone | ✅ | ❌ | ✅ | ❌ | ✅ |
| 剪贴板 | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ |

## 工程

| 能力 | cu (ours) | Anthropic Computer Use | Ghost OS | agent-desktop | usecomputer | axcli |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| 架构 | CLI 单二进制 | API tool | MCP daemon | CLI 多crate | CLI N-API | CLI |
| 语言 | Rust | Python | Swift | Rust | Zig+TS | Rust |
| 二进制 | 1.2MB | — | ~50MB+3GB | <15MB | ~5MB | ~3MB |
| 依赖 | 零 | Python | macOS | 零 | Node.js | 零 |
| 延迟 | <10ms | 3-8s | <100ms | ~200ms | ~100ms | ~100ms |
| JSON/Human | ✅ auto | JSON | JSON | JSON | JSON | 混合 |
| 权限引导 | ✅ 双检查 | ❌ | 文档 | ❌ | ❌ | ❌ |
| Token 效率 | ✅ 文本优先 | ❌ ~1400/截图 | ✅ | ✅ | ❌ | ✅ |

---

*Last updated: 2026-04-03 — Added: scroll, double-click, hover, drag, wait, modifiers, OCR, batch AX, 14-step click chain*
