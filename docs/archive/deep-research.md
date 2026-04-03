# Computer Pilot — Deep Research: Complete Computer Control via CLI

> Date: 2026-04-02 | Status: Research Synthesis
> Sources: 10 parallel research agents, 200+ web sources, 50+ projects analyzed

---

## Executive Summary

**The problem:** Current computer-use agents are toys. Best-in-class (Claude Opus 4.6) scores 72.7% on OSWorld — meaning it fails ~1 in 4 tasks. Real users on Hacker News say: *"less capability, more reliability, please."*

**The root cause:** Everyone is doing it wrong. The dominant approach — screenshot → vision model → coordinate click — is slow (3-10s/step), expensive (~$0.05-0.30/action), fragile (coordinate drift, dynamic UI), and wastes tokens on pixel data.

**The insight:** 80-90% of computer tasks can be completed WITHOUT ever taking a screenshot, if you use the right control layer for each task.

**The vision:** computer-pilot is not another computer-use demo. It's an **operating system control layer** — a CLI that gives ANY agent complete, reliable, efficient control over a user's computer.

---

## I. The 5-Layer Control Hierarchy

This is the most important architectural insight from our research. Instead of a one-size-fits-all approach, the optimal strategy is a **priority cascade** — try the fastest/cheapest/most reliable method first, fall back only when necessary.

```
Layer 0: Direct CLI / System API          ← instant, deterministic, zero tokens
Layer 1: AppleScript / JXA Semantic       ← instant, deterministic, minimal tokens
Layer 2: Accessibility Tree (AX)          ← <500ms, structured, ~1000 tokens
Layer 3: CGEvent (coordinate-based)       ← <100ms, needs coordinates from higher layer
Layer 4: Screenshot + Vision Model        ← 2-10s, expensive, universal fallback
```

### Quantitative comparison

| Layer | Accuracy | Cost/Action | Latency | Tokens/Action | Coverage |
|-------|----------|-------------|---------|---------------|----------|
| 0: CLI/API | ~99% | ~$0 | <10ms | 50-200 | ~25-35% of tasks |
| 1: AppleScript | ~95% | ~$0 | <50ms | 50-200 | ~20-30% additional |
| 2: AX Tree | ~80-90% | $0.01-0.05 | 100-500ms | 500-2,000 | ~30-40% additional |
| 3: CGEvent | ~70-80% | $0.01 | <100ms | 100-500 | coordinate-dependent |
| 4: Screenshot | ~60-70% | $0.05-0.30 | 2-10s | 5,000-30,000 | ~100% (universal) |

**Key insight:** By cascading through layers 0→1→2 before touching the screen, we handle ~70-80% of tasks at near-zero cost and near-perfect reliability. Layer 4 (screenshots) becomes a rare fallback, not the primary approach.

### Evidence

- **CLI-Anything** (26K+ stars): Proved CLI interfaces are 100% reliable, sub-second, minimal tokens for creative/productivity apps
- **Microsoft UFO2**: GUI+API hybrid achieves 24.5% vs 16.3% GUI-only (+8.2pp), with 51.5% fewer LLM calls
- **Playwright MCP CLI**: Accessibility approach uses 27K tokens vs 114K tokens (4x reduction) vs screenshot approach
- **DOM Intelligence (rtrvr.ai)**: Structured DOM achieves 81% accuracy vs 60% for screenshot-based, at $0.12 vs $0.50-3.00/task
- **macOS Automator MCP**: 200+ AppleScript recipes prove semantic scripting covers vast majority of common tasks

---

## II. What Users Actually Need (and Where Current Tools Fail)

### Task demand ranking (from surveys + forum analysis)

| Tier | Tasks | User Demand | Current Agent Capability | Gap |
|------|-------|-------------|------------------------|-----|
| 1 | Research & summarization | 58% | Strong | Small |
| 1 | Workflow automation | 53.5% | **Weak** (GUI-dependent) | **Large** |
| 1 | Email/calendar/communication | High | Fails at auth | **Large** |
| 2 | Decision support (comparisons, planning) | 45% | Moderate | Medium |
| 2 | Document creation/transformation | High | Decent for text, poor for design | Medium |
| 2 | Data entry & transfer between apps | High | **Fails at multi-app** | **Large** |
| 3 | File management (batch, convert, organize) | Medium | Good via CLI, weak via GUI | Medium |
| 3 | System admin (settings, troubleshooting) | Medium | Permission barriers | Large |
| 3 | Media (photo/video/audio editing) | Medium | Very weak (canvas) | Very Large |

### The 7 deadly failure modes (from benchmark analysis)

1. **GUI grounding errors** — 28-pixel border shift can cause 193-pixel coordinate miss
2. **UI element hallucination** — agents "see" buttons that don't exist
3. **Context drift** — by step 15/20, original goal is lost from context window
4. **Silent failure** — agents proceed with corrupted state instead of asking for help
5. **Error compounding** — early mistakes cascade non-linearly
6. **Dynamic content** — loading spinners, animations, AJAX break assumptions
7. **Authentication collapse** — agents get logged out mid-task

### The "last mile" problems (tasks that almost work but fail at the end)

| Problem | Why It Fails | computer-pilot Solution |
|---------|-------------|------------------------|
| Payment processing | Safety policies block financial data entry | Out of scope (by design) |
| Account creation | CAPTCHAs, email verification loops | Human handoff at verification |
| Two-factor auth | SMS/TOTP codes are hard stops | Pause + request user provide OTP |
| File upload dialogs | OS-level native dialogs | **AppleScript + AX tree** can handle these |
| Print operations | Unpredictable dialog variations | **`lp`/`lpr` CLI bypass** |
| System permission dialogs | TCC requires human click (by design) | Cannot automate — `cu setup` pre-configures |
| sudo/admin password | Security boundary | GUI password prompt pattern |
| App installation | Gatekeeper, DMG mount, drag-to-Applications | **`brew install` via CLI (Layer 0)** |

### Critical user insight

> "I want 99% reliability on narrow tasks, not 40% reliability on everything." — HN consensus

This means: **don't try to do everything through one mechanism**. Use the best tool for each job. That's what the 5-layer hierarchy delivers.

---

## III. Radical Approaches Worth Adopting

### 1. CLI-Anything: Make Software Agent-Native (MUST STUDY)

**Project:** [HKUDS/CLI-Anything](https://github.com/HKUDS/CLI-Anything) — 26.5K stars

Instead of making agents see and click apps, **make apps speak CLI.** A 7-phase automated pipeline analyzes app source code → generates Click CLI wrappers → publishes to PyPI. 20+ apps supported (Blender, GIMP, LibreOffice, OBS, Audacity, etc.), 1,839 tests at 100% pass rate.

**Why it matters for computer-pilot:** We can integrate CLI-Anything CLIs as Layer 0 plugins. When an agent needs to edit an image in GIMP, instead of AX tree + screenshot, it calls `gimp-cli apply-filter --filter gaussian-blur --radius 5 image.png`. Sub-second, deterministic, zero screenshots.

**How to adopt:**
- `cu` detects if a CLI-Anything CLI exists for the target app
- If yes, delegates to it (fastest path)
- If no, falls back to Layer 1-4
- Agent can install new CLIs on demand: `pip install gimp-cli`

### 2. OpenCLI: Turn Websites Into CLI Commands

**Project:** [jackwener/opencli](https://github.com/jackwener/opencli) — 10.8K stars

Transforms ANY website/Electron app into a CLI. 66+ adapters (Twitter/X, Reddit, YouTube, GitHub, Slack, Notion, Bilibili, Zhihu, Xiaohongshu). Uses Chrome's existing login cookies — credentials never leave the browser.

**Why it matters:** For web-based tasks, instead of screenshot → vision → coordinate click in a browser, the agent calls `opencli twitter post "hello world"`. Deterministic, fast, auth handled.

**How to adopt:**
- `cu web <service> <action>` delegates to opencli when available
- Complements browser-pilot: `bp` for interactive browsing, `opencli` for structured web actions

### 3. Self-Learning Recipes (Ghost OS Pattern)

**Project:** [ghostwright/ghost-os](https://github.com/ghostwright/ghost-os) — macOS MCP server

"A frontier model figures out the workflow once. A small model runs it forever." Records user actions via CGEvent tap, AI synthesizes parameterized JSON recipes, replays with parameter substitution.

**Why it matters:** Any workflow the user does repeatedly can be learned once (expensive) and replayed forever (near-zero cost). This is the path to handling the long tail of app-specific tasks.

**How to adopt:**
- `cu learn start` — begin recording user actions
- `cu learn stop` — AI synthesizes recipe
- `cu recipe run "rename-files" --pattern "*.jpg" --prefix "vacation-"`
- Recipes are JSON, shareable, versionable

### 4. Self-Evolving Skills (OpenSpace Pattern)

**Project:** [HKUDS/OpenSpace](https://github.com/HKUDS/OpenSpace)

Agents don't just execute — they **learn reusable skills** that compound over time. Results: quality 40.8% → 70.8%, token usage -46%, 165 skills evolved across 50 tasks.

Three evolution modes: FIX (auto-repair broken skills), DERIVED (compose new skills from existing), CAPTURED (extract patterns from successful execution).

**How to adopt:**
- Skills directory: `~/.computer-pilot/skills/`
- After successful multi-step task, offer to save as skill
- Skills auto-evolve when execution fails (FIX mode)

### 5. Hybrid API+GUI (UFO2/UFO3 Pattern)

**Microsoft UFO2** proved that combining native APIs with GUI fallback is dramatically better:
- 24.5% vs 16.3% on Office tasks (GUI+API vs GUI-only)
- 51.5% fewer LLM calls via speculative multi-action
- 62% of failures are control detection failures — hybrid detection is the biggest lever

**How to adopt:** This IS our 5-layer hierarchy. Computer-pilot embodies this principle.

---

## IV. macOS: The Complete Control Surface

Our macOS deep-dive (see `macos-control-bible.md`) revealed that macOS provides an extraordinarily rich programmatic control surface — far richer than any existing tool exploits.

### What no existing tool does today

| Capability | agent-desktop | Ghost OS | axcli | cliclick | macOS Automator MCP | **computer-pilot** |
|-----------|:---:|:---:|:---:|:---:|:---:|:---:|
| AX Tree perception | Yes | Yes | Yes | No | No | **Yes** |
| CGEvent input | Yes | Yes | Yes | Yes | No | **Yes** |
| AppleScript/JXA | No | No | No | No | Yes (recipes) | **Yes** |
| System settings (net, display, sound) | No | No | No | No | Partial | **Yes** |
| Shortcuts integration | No | No | No | No | No | **Yes** |
| File deep ops (tags, Spotlight, xattr) | No | No | No | No | Partial | **Yes** |
| Multi-format clipboard | Partial | No | No | No | Partial | **Yes** |
| Screenshot + OCR | No | Yes (3GB model) | Yes | No | No | **Yes (built-in Vision, 0 download)** |
| Browser JS execution | No | Yes (CDP) | No | No | Yes | **Yes (via AppleScript)** |
| Drag & drop | Yes | No | No | Yes | No | **Yes** |
| Self-learning recipes | No | Yes | No | No | No | **Yes** |
| **Unified CLI for ALL** | **No** | **No** | **No** | **No** | **No** | **Yes** |

### The AppleScript / JXA superpower

19 apps on this system have AppleScript dictionaries with CRUD operations. For these apps, semantic scripting is **orders of magnitude better** than UI automation:

| Task | AX Tree approach | AppleScript approach |
|------|-----------------|---------------------|
| Send email | Focus Mail → click Compose → find To field → type → find Subject → type → find Body → type → click Send (8+ steps, fragile) | `tell app "Mail" to make new outgoing message with properties {subject:"Hi", content:"Body", visible:true}; send` (1 step, reliable) |
| Create calendar event | Navigate Calendar → click + → find fields → type date/time/title (10+ steps) | `tell app "Calendar" to make new event with properties {summary:"Meeting", start date:date "..."}` (1 step) |
| Get Safari URL | AX tree of Safari → find address bar → read value (3 steps, fragile) | `tell app "Safari" to get URL of current tab of window 1` (instant) |
| Execute JS in Chrome | Cannot via AX | `tell app "Chrome" to execute front window's active tab javascript "..."` (instant) |

### JXA's ObjC bridge — the hidden nuclear option

JXA (JavaScript for Automation) can call **any Cocoa/CoreFoundation API** without compiling Swift:

```javascript
// Via: osascript -l JavaScript -e '...'
ObjC.import("AppKit");
ObjC.import("CoreLocation");
ObjC.import("CoreBluetooth");
// Access ANY framework. Full native power from a shell command.
```

This means computer-pilot's Swift helper doesn't need to cover every API. For rare/niche operations, the agent can generate JXA on the fly.

### System control surface (all via CLI, zero GUI needed)

| Domain | CLI Tool | Operations |
|--------|---------|------------|
| Network | `networksetup` | 80+ commands: Wi-Fi, DNS, proxy, VPN, locations, VLAN |
| Preferences | `defaults` | Read/write ANY app's preferences (hundreds of hidden settings) |
| Keychain | `security` | Find/add/delete passwords, manage certificates |
| Power | `pmset`, `caffeinate` | Sleep, wake, schedule, prevent sleep, battery info |
| Bluetooth | `blueutil` (brew) | On/off, pair, connect, disconnect |
| Display | `displayplacer` (brew) | Resolution, arrangement, rotation |
| Audio | `osascript` | Volume, mute, input level |
| Dark mode | `osascript` | Toggle via System Events |
| File search | `mdfind` | Spotlight search (instant, powerful) |
| File metadata | `mdls`, `xattr`, `tag` | Tags, comments, extended attributes |
| Images | `sips` | Resize, convert, rotate, flip — 12+ operations |
| Documents | `textutil` | Convert between doc/docx/rtf/html/txt |
| PDF | `python3 + Quartz` | Page count, extract text, merge/split |
| Audio | `afplay`, `afconvert`, `say` | Play, convert, text-to-speech |
| App management | `open`, `lsappinfo`, `mas` | Launch, info, App Store CLI |
| Disk | `diskutil`, `hdiutil` | Format, partition, mount DMG |
| Services | `launchctl` | Start/stop/manage daemons |
| Users | `dscl`, `sysadminctl` | User management |
| Time Machine | `tmutil` | Backup management |
| Shortcuts | `shortcuts run` | Execute any user-created Shortcut |
| Print | `lp`, `lpstat` | Print, check printer status |

### What CANNOT be done programmatically

| Limitation | Reason | Workaround |
|-----------|--------|------------|
| Multi-touch gestures | Private MultitouchSupport.framework | Use keyboard shortcuts instead |
| Create Shortcuts programmatically | No API, can only import .shortcut files | Pre-build and distribute .shortcut files |
| Focus/DND toggle from CLI | Shortcuts only | `shortcuts run "Toggle Focus"` |
| Night Shift | Private CoreBrightness framework | JXA ObjC bridge can call it |
| Space/Desktop management | Private SkyLight APIs, requires SIP modification | yabai for advanced users |
| Read notification history | SIP-protected database | Cannot — design around it |
| TCC permission dialogs | Requires human click by design | `cu setup` pre-configures everything |

---

## V. Command Design: Beyond the MVP

The current research.md defines 6 MVP commands. Here's the expanded vision based on the 5-layer hierarchy:

### Tier 1: Core Observation-Action Loop (MVP)

```bash
cu apps                         # list running apps (Layer 0)
cu snapshot "Finder"            # AX tree with [ref] numbers (Layer 2)
cu click <ref|x,y>             # click element or coordinate (Layer 2→3)
cu type <ref|-> "text"          # type into element or focus (Layer 2→3)
cu key <combo>                  # keyboard shortcut (Layer 3)
cu screenshot                   # capture screen (Layer 4)
```

### Tier 2: System Control (Layer 0 — no GUI needed)

```bash
cu system volume 50             # set volume to 50%
cu system volume mute           # mute
cu system wifi on|off           # toggle Wi-Fi
cu system wifi connect "SSID"   # connect to network
cu system bluetooth on|off      # toggle Bluetooth
cu system dark-mode on|off      # toggle dark mode
cu system brightness 70         # set display brightness
cu system sleep                 # put to sleep
cu system open "https://..."    # open URL in default browser
cu system open "/path/to/file"  # open file in default app
cu system notify "title" "body" # send notification
```

### Tier 3: App Scripting (Layer 1 — AppleScript semantic)

```bash
cu script "Mail" send --to "x@y.com" --subject "Hi" --body "Hello"
cu script "Calendar" create-event --title "Meeting" --date "2026-04-03 14:00"
cu script "Safari" get-url                    # get current URL
cu script "Chrome" exec-js "document.title"   # execute JavaScript
cu script "Finder" create-folder "/tmp/new"
cu script "Finder" tag "/path/to/file" "Important"
cu script "Notes" create --title "Meeting Notes" --body "..."
cu script "Reminders" add "Buy groceries" --due "tomorrow"
cu script "Music" play|pause|next|previous
cu script raw 'tell app "..." to ...'         # raw AppleScript
```

### Tier 4: File Operations (Layer 0)

```bash
cu file search "quarterly report"             # Spotlight search (mdfind)
cu file info "/path/to/file"                  # metadata (mdls)
cu file tag "/path" add "Important"           # Finder tags
cu file convert image "/path.png" --to jpeg   # sips conversion
cu file convert doc "/path.docx" --to html    # textutil conversion
cu file pdf text "/path.pdf"                  # extract text
cu file trash "/path/to/file"                 # move to Trash
```

### Tier 5: Clipboard (Layer 0-1)

```bash
cu clipboard get                              # read text
cu clipboard set "text to copy"               # write text
cu clipboard get --type html                  # read as HTML
cu clipboard set-file "/path/to/file"         # copy file reference
cu clipboard info                             # list available types
```

### Tier 6: Window Management (Layer 2)

```bash
cu window list                                # all windows with refs
cu window focus <ref|"App Name">              # bring to front
cu window resize <ref> 800x600                # resize
cu window move <ref> 100,100                  # move
cu window minimize <ref>                      # minimize
cu window fullscreen <ref>                    # toggle fullscreen
cu window close <ref>                         # close
```

### Tier 7: Advanced (Layer 1-4)

```bash
cu ocr                                        # screenshot + built-in Vision OCR
cu ocr --region 100,100,400,300               # OCR specific region
cu wait --text "Done"                         # wait for text on screen
cu wait --gone <ref>                          # wait for element to disappear
cu drag <ref|x,y> to <ref|x,y>               # drag and drop
cu scroll <ref|-> up|down <amount>            # scroll
cu menu "Edit" "Find" "Find..."              # navigate menu hierarchy
cu learn start|stop                           # record/synthesize recipe
cu recipe run "name" --arg1 "value"           # run saved recipe
```

### Tier 8: Permissions & Setup

```bash
cu setup                                      # guided permission wizard
cu permissions                                # check permission status
cu doctor                                     # diagnose common issues
```

### Design principles (from browser-pilot lessons)

1. **Auto-snapshot after every action** — `cu click 3` returns the updated AX tree, not just `{"ok": true}`
2. **JSON when piped, human-readable when TTY** — detect `process.stdout.isTTY`
3. **Refs are sequential integers, DFS order, interactive-only** — `[1]` `[2]` `[3]`, refresh after every action
4. **Errors include exactly one actionable hint** — `"Ref [99] not found. Run 'cu snapshot \"Finder\"' to refresh."`
5. **Never fail silently** — always `{"ok": false, "error": "...", "hint": "..."}`
6. **Daemon architecture** — Swift helper is long-running, CLI process is ephemeral, communicates via Unix socket
7. **Text first, images only when needed** — AX snapshot ~1000 tokens vs screenshot ~1400 tokens but with far more actionable info

---

## VI. Architecture: A True Platform

### Three integration surfaces (covers every agent)

```
Surface 1: CLI binary (`cu`)              — works with ANY agent, zero setup
Surface 2: MCP server (`cu mcp serve`)    — structured discovery, typed schemas
Surface 3: Agent plugins (SKILL.md)       — agent-specific guides (Claude Code, Codex, etc.)
```

All three call the same core engine. CLI is the single source of truth.

### Why CLI-first beats MCP-first

| Dimension | CLI Wins | MCP Wins |
|-----------|----------|----------|
| Latency | Process spawn < MCP handshake | - |
| Token cost | No schema overhead | - |
| LLM fluency | Trained on billions of shell examples | - |
| Universality | 100% of agents can spawn processes | ~80% support MCP |
| Testability | `bash` scripts can test everything | - |
| Debuggability | Humans can run commands directly | - |
| Discovery | - | Excellent (`tools/list`) |
| Type safety | - | Strong (JSON Schema) |
| Security governance | - | Better (scoped, auditable) |

**Conclusion:** CLI is primary, MCP is the discovery/governance layer. Both call the same engine.

### Full architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        AGENT LAYER                          │
│  Claude Code │ Codex │ Gemini CLI │ Open Interpreter │ Any  │
└───────┬──────┴───┬───┴─────┬──────┴────────┬─────────┴──┬──┘
        │          │         │               │            │
   Bash tool    shell     shell           subprocess   process
        │          │         │               │            │
        ▼          ▼         ▼               ▼            ▼
┌─────────────────┐  ┌────────────────┐  ┌───────────────────┐
│  CLI (`cu`)     │  │  MCP Server    │  │  Plugin (SKILL.md) │
│  stdin/stdout   │  │  JSON-RPC      │  │  Agent-specific    │
│  exit codes     │  │  tool schemas  │  │  instructions      │
└────────┬────────┘  └───────┬────────┘  └───────────────────┘
         │                   │
         ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│                    DAEMON (persistent Swift process)          │
│                                                              │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌──────────┐│
│  │  Router   │  │  State    │  │  Safety   │  │  Audit   ││
│  │  (5-layer │  │  Manager  │  │  Engine   │  │  Logger  ││
│  │  cascade) │  │  (refs,   │  │  (allow/  │  │          ││
│  │           │  │   focus,  │  │   block/  │  │          ││
│  │           │  │   cache)  │  │   confirm)│  │          ││
│  └─────┬─────┘  └───────────┘  └───────────┘  └──────────┘│
│        │                                                     │
│        ▼                                                     │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              CONTROL LAYERS                          │    │
│  │                                                      │    │
│  │  Layer 0: CLI/API Engine                             │    │
│  │    osascript, defaults, networksetup, diskutil,      │    │
│  │    security, mdfind, sips, textutil, shortcuts, etc. │    │
│  │                                                      │    │
│  │  Layer 1: AppleScript/JXA Semantic Engine            │    │
│  │    App dictionaries, System Events, ObjC bridge      │    │
│  │                                                      │    │
│  │  Layer 2: AX Tree Engine                             │    │
│  │    AXUIElement, batch reads, 15-step click chain,    │    │
│  │    ref assignment, element caching                   │    │
│  │                                                      │    │
│  │  Layer 3: CGEvent Engine                             │    │
│  │    Mouse click/move/drag, keyboard events,           │    │
│  │    coordinate scaling for Retina                     │    │
│  │                                                      │    │
│  │  Layer 4: Vision Engine                              │    │
│  │    ScreenCaptureKit (5-15ms), Vision OCR (200ms),    │    │
│  │    coordinate extraction from vision model response  │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              EXTENSION POINTS                        │    │
│  │  CLI-Anything CLIs │ OpenCLI adapters │ Recipes      │    │
│  │  Skills library    │ App plugins      │ Custom JXA   │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### Smart routing logic

When the agent calls `cu click 3`, the router:

1. Looks up ref [3] in the ref registry → gets element info (app, role, name, AX path)
2. **Is it a scriptable action?** (e.g., "Save" in a scriptable app) → Layer 1 (AppleScript `save document 1`)
3. **Is there an AX action?** (AXPress, AXConfirm, AXOpen, AXPick) → Layer 2 (15-step chain)
4. **Does AX give us coordinates?** → Layer 3 (CGEvent click at element center)
5. **Nothing worked?** → Layer 4 (screenshot + ask agent for coordinates)

This cascading happens transparently. The agent just says `cu click 3` and gets the result.

### Safety model

| Layer | Mechanism | Example |
|-------|-----------|---------|
| **Permission gating** | macOS TCC (3 permissions) | `cu setup` wizard |
| **Action classification** | Safe / Confirm / Block | `cu key cmd+q` → confirm; `cu apps` → safe |
| **App scope** | Allowlist/blocklist config | Block "Keychain Access", "System Settings" |
| **Rate limiting** | Max actions/second | Default 10 actions/s |
| **Audit logging** | Every action logged to `~/.computer-pilot/audit.log` | JSON with timestamp, command, element, agent, result |
| **Destructive detection** | Heuristic on element names | Button named "Delete", "Remove", "Erase" → confirm |

---

## VII. The Bold Capabilities: What Makes This Not a Toy

### 1. Cross-app workflows (the hardest unsolved problem)

Current agents fail at multi-app tasks because each app is an isolated interaction. computer-pilot solves this by providing a **unified state layer**:

```bash
# Example: "Copy data from Numbers, paste into Mail"
cu focus "Numbers"
cu snapshot "Numbers"                          # see the spreadsheet
cu click 5                                     # select cell range
cu key cmd+c                                   # copy
cu focus "Mail"
cu script "Mail" compose --to "boss@co.com" --subject "Report"
cu key cmd+v                                   # paste into email body
cu script "Mail" send                          # send
```

The agent orchestrates, computer-pilot handles the per-app interactions. Clipboard is the universal data bus.

### 2. System administration (zero GUI needed)

```bash
# "Set up Wi-Fi, change DNS, enable dark mode, and set volume"
cu system wifi connect "OfficeNet"
cu system dns set 8.8.8.8 8.8.4.4
cu system dark-mode on
cu system volume 30
```

No screenshots. No AX tree. No vision model. Just deterministic CLI commands.

### 3. Intelligent file management

```bash
# "Find all PDF invoices from 2025 and tag them as 'Tax 2025'"
cu file search "kind:pdf invoice 2025"         # Spotlight search
cu file tag "/path/to/invoice1.pdf" add "Tax 2025"
cu file tag "/path/to/invoice2.pdf" add "Tax 2025"
# ... agent iterates through results
```

### 4. Browser integration (without screenshots)

```bash
# "Get the title and URL of the current Safari tab"
cu script "Safari" get-url                     # → "https://..."
cu script "Safari" get-title                   # → "Page Title"

# "Execute JavaScript to extract data from Chrome"
cu script "Chrome" exec-js "JSON.stringify(Array.from(document.querySelectorAll('table tr')).map(r => Array.from(r.cells).map(c => c.textContent)))"
```

### 5. Self-learning workflows

```bash
# User: "Watch me do this, then do it again whenever I ask"
cu learn start
# User performs actions manually...
cu learn stop --name "weekly-report"
# Later:
cu recipe run "weekly-report" --week "2026-W14"
```

### 6. Complete media pipeline

```bash
# "Resize all images in ~/Photos/vacation/ to 1024px wide, convert to JPEG"
cu file search "kind:image" --path ~/Photos/vacation/
# For each result:
cu file convert image "/path/to/photo.heic" --to jpeg --max-width 1024 --out "/output/"
```

All via `sips` (built into macOS). No Photoshop needed.

---

## VIII. Ecosystem: How Third Parties Extend It

### Plugin architecture (proven by browser-pilot)

```
~/.computer-pilot/
  plugins/
    photoshop/
      SKILL.md          # Agent instructions for Photoshop tasks
      workflows/        # Common Photoshop workflows
      recipes.json      # Pre-built action sequences
    slack/
      SKILL.md
      cli-anything/     # Optional CLI-Anything generated CLI
  skills/               # Self-learned skills (OpenSpace pattern)
    rename-photos.json
    weekly-report.json
  audit.log             # Action audit trail
  config.json           # Safety settings, allowed apps, etc.
```

### Integration with CLI-Anything Hub

CLI-Anything generates CLIs for individual apps. computer-pilot consumes them as plugins:

```bash
cu plugin add cli-anything:gimp     # installs gimp-cli
cu plugin add cli-anything:blender  # installs blender-cli
# Now `cu` detects these and routes through them for Layer 0 operations
```

### Integration with OpenCLI

```bash
cu plugin add opencli:twitter       # installs twitter adapter
cu plugin add opencli:github        # installs github adapter
cu web twitter post "Hello from computer-pilot!"
```

---

## IX. Cross-Platform Strategy (Future)

### Current focus: macOS first (right decision)

- Richest programmatic control surface (AppleScript, AX API, system CLIs)
- Best single-platform coverage potential (~90% of tasks)
- Your development environment

### Windows expansion (when ready)

| macOS Equivalent | Windows Equivalent |
|-----------------|-------------------|
| AXUIElement | UI Automation (UIA) |
| AppleScript/JXA | COM Automation / PowerShell |
| CGEvent | Win32 SendInput |
| ScreenCaptureKit | Desktop Duplication API |
| Vision OCR | Windows.Media.Ocr |
| `defaults` | Registry (`reg`) |
| `networksetup` | `netsh` |
| `diskutil` | `diskpart` |
| `pmset` | `powercfg` |
| `security` | `cmdkey` (Credential Manager) |
| `shortcuts run` | Power Automate Desktop |
| `mdfind` | Windows Search / `where` |
| `sips` | No equivalent (ImageMagick/ffmpeg) |
| `osascript` | `cscript`/`wscript` (VBScript/JScript) |

### Linux expansion

| macOS Equivalent | Linux Equivalent |
|-----------------|-----------------|
| AXUIElement | AT-SPI2 (D-Bus) |
| AppleScript | D-Bus methods (app-specific) |
| CGEvent | ydotool (X11+Wayland) |
| ScreenCaptureKit | scrot (X11) / grim (Wayland) |
| Vision OCR | Tesseract |

**Wayland fragmentation** is the biggest challenge on Linux. No unified window management API.

### Architecture for cross-platform

```
cu (TypeScript CLI, platform-agnostic)
 │
 │  Detects platform, spawns appropriate helper
 ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│ macOS Helper│  │ Win Helper  │  │ Linux Helper│
│ (Swift)     │  │ (C#/.NET)   │  │ (Rust/C)    │
│             │  │             │  │             │
│ AX + AS +   │  │ UIA + COM + │  │ AT-SPI +    │
│ CGEvent +   │  │ SendInput + │  │ ydotool +   │
│ SCKit +     │  │ OCR + etc.  │  │ scrot + etc.│
│ Vision      │  │             │  │             │
└─────────────┘  └─────────────┘  └─────────────┘
```

Same `cu` commands, platform-specific backends. Agent doesn't need to know which platform.

---

## X. Competitive Positioning

### What makes computer-pilot unique

| Feature | agent-desktop | Ghost OS | Anthropic CU | OpenAI CUA | **computer-pilot** |
|---------|:---:|:---:|:---:|:---:|:---:|
| CLI interface | Yes | No (MCP) | No (API) | No (Product) | **Yes** |
| MCP server | No | Yes | No | No | **Yes** |
| AX-first perception | Yes | Yes | No (vision) | No (vision) | **Yes** |
| AppleScript semantic | No | No | No | No | **Yes** |
| System-level CLI control | No | No | No | No | **Yes** |
| Vision OCR fallback | No | 3GB model | Built-in | Built-in | **Built-in (0 download)** |
| Self-learning recipes | No | Yes | No | No | **Yes** |
| CLI-Anything integration | No | No | No | No | **Yes** |
| Plugin ecosystem | No | No | No | No | **Yes** |
| Cross-platform | No | No | Yes (remote) | Web only | **Planned** |
| Token efficiency | Good | Good | Poor (vision) | Poor (vision) | **Excellent** |
| Any-agent compatible | Partial | Partial | API only | Product only | **Yes (CLI universal)** |
| npm distribution | Yes | brew | N/A | N/A | **Yes** |
| Pairs with browser-pilot | No | No | No | No | **Yes** |

### The positioning statement

> computer-pilot is the control layer between AI agents and your operating system. It gives any agent — Claude Code, Codex, Gemini CLI, or your custom tool — the ability to do everything you can do on your computer, reliably, efficiently, and safely.

---

## XI. Key Research Sources

### Most Important Projects to Study

| Project | Why | Link |
|---------|-----|------|
| **CLI-Anything** | Core philosophy: make apps speak CLI, not screenshots | github.com/HKUDS/CLI-Anything |
| **agent-desktop** | Best AX implementation: 15-step click chain, ref system | github.com/lahfir/agent-desktop |
| **Ghost OS** | Self-learning recipes, AXorcist Swift wrapper | github.com/ghostwright/ghost-os |
| **axcli** | Playwright-style AX locators (CSS selectors for AX) | github.com/andelf/axcli |
| **macOS Automator MCP** | 200+ AppleScript recipes, MCP design | github.com/steipete/macos-automator-mcp |
| **Microsoft UFO2** | Hybrid API+GUI architecture, quantitative proof | github.com/microsoft/UFO |
| **OpenSpace** | Self-evolving skill engine | github.com/HKUDS/OpenSpace |
| **browser-pilot** | Proven CLI architecture, plugin system, daemon pattern | (our own project) |

### Key Papers

| Paper | Contribution |
|-------|-------------|
| OSWorld (NeurIPS 2024) | The benchmark: 369 real-computer tasks, human baseline 72.36% |
| UFO2 (arXiv 2504.14603) | Proved hybrid API+GUI > GUI-only (+8.2pp), 51.5% fewer LLM calls |
| CLI-Anything | 100% pass rate vs GUI-agent fragility for creative apps |
| ShowUI (CVPR 2025) | 2B vision model, 90% fewer hallucinated actions |
| Survey: Agents for Computer Use (arXiv 2501.16150) | Comprehensive taxonomy, 6 major gaps identified |

### Industry Data Points

- OSWorld best score: 72.7% (Claude Opus 4.6) — matches human 72.36%
- OSWorld-Verified: 75.0% (GPT-5.4) — exceeds human level
- Agents take 1.4-2.7x more steps than humans (OSWorld-Human study)
- 80-90% of AI projects never leave pilot phase
- Only 14% of enterprises at production scale with agents
- MCP: 97M+ monthly SDK downloads, 5800+ community servers
- Claude Code: ~4% of GitHub commits (doubling monthly)

---

## XII. Implementation Priority

### Phase 0: Foundation (Week 1-2)
- Swift daemon with JSON-over-stdin/stdout protocol
- TypeScript CLI (`cu`) with npm distribution
- Unix socket IPC between CLI and daemon
- `cu setup` permission wizard

### Phase 1: Core Loop (Week 3-4)
- `cu apps`, `cu snapshot`, `cu click`, `cu type`, `cu key`, `cu screenshot`
- AX tree perception with ref system (learn from agent-desktop)
- 15-step AX click chain
- Auto-snapshot after actions
- CGEvent fallback for coordinate clicks

### Phase 2: Layer 0-1 Powers (Week 5-6)
- `cu system` commands (volume, wifi, bluetooth, dark-mode, etc.)
- `cu script` AppleScript/JXA semantic engine
- `cu file` operations (search, tag, convert)
- `cu clipboard` operations

### Phase 3: Vision & Intelligence (Week 7-8)
- `cu ocr` via built-in Vision framework
- `cu screenshot` with intelligent region capture
- `cu wait` commands
- `cu learn` / `cu recipe` self-learning system

### Phase 4: Platform Layer (Week 9-10)
- MCP server (`cu mcp serve`)
- Plugin system and SKILL.md for agents
- CLI-Anything integration
- Safety engine (action classification, audit logging)
- `cu window` management commands

### Phase 5: Ecosystem (Ongoing)
- Plugin registry
- Community recipes
- Cross-platform expansion
- OpenCLI integration
- Skill evolution (OpenSpace pattern)
