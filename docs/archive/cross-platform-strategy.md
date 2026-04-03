# Computer Pilot — Cross-Platform Strategy

> Research completed: 2026-04-02
> Goal: Complete computer control across macOS, Windows, Linux (and optionally mobile/remote)

---

## Table of Contents

1. [Platform Capability Matrix](#1-platform-capability-matrix)
2. [Windows Complete Control](#2-windows-complete-control)
3. [Linux Complete Control](#3-linux-complete-control)
4. [Cross-Platform Architecture Decision](#4-cross-platform-architecture-decision)
5. [Mobile Platforms](#5-mobile-platforms)
6. [Remote/Cloud Control](#6-remotecloud-control)
7. [Implementation Roadmap](#7-implementation-roadmap)

---

## 1. Platform Capability Matrix

### Perception Layer (Reading the Screen)

| Capability | macOS | Windows | Linux (X11) | Linux (Wayland) |
|---|---|---|---|---|
| **Accessibility Tree** | AXUIElement API | UI Automation (UIA) | AT-SPI2 via D-Bus | AT-SPI2 via D-Bus |
| **Tree Quality** | Excellent (AppKit/SwiftUI) | Excellent (WPF/WinForms/UWP) | Good (GTK/Qt) | Good (GTK/Qt) |
| **Screenshot** | ScreenCaptureKit (5-15ms) | GDI BitBlt / DXGI | X11 + XShm | Compositor-specific |
| **OCR** | Vision Framework (built-in) | Windows.Media.OCR (built-in) | Tesseract (install) | Tesseract (install) |
| **Window List** | NSWorkspace / CGWindowList | EnumWindows / UIA | wmctrl / xdotool | Compositor-specific |

### Action Layer (Controlling the Computer)

| Capability | macOS | Windows | Linux (X11) | Linux (Wayland) |
|---|---|---|---|---|
| **Semantic Click** | AXPress/AXConfirm actions | UIA InvokePattern | AT-SPI2 DoAction | AT-SPI2 DoAction |
| **Coordinate Click** | CGEvent | SendInput | xdotool | ydotool (uinput) |
| **Keyboard Input** | CGEvent + keyboardSetUnicodeString | SendInput | xdotool | ydotool |
| **Text Entry** | AXSetValue | UIA ValuePattern | AT-SPI2 SetText | AT-SPI2 SetText |
| **Window Mgmt** | AXRaise/AXMove/AXResize | UIA WindowPattern | wmctrl | Compositor-specific |
| **Drag & Drop** | CGEvent (mouseDown/move/up) | SendInput | xdotool | ydotool (limited) |
| **Scroll** | CGEvent (scrollWheel) | SendInput | xdotool | ydotool |

### System Control Layer

| Capability | macOS | Windows | Linux |
|---|---|---|---|
| **Shell** | zsh/bash | PowerShell / CMD | bash/zsh |
| **Package Mgmt** | brew | winget / choco | apt/dnf/pacman |
| **Service Mgmt** | launchctl | sc.exe / services.msc | systemctl |
| **Settings** | defaults / System Settings | ms-settings: URI / registry | gsettings / kwriteconfig |
| **Network** | networksetup | netsh / nmcli equivalent | nmcli |
| **Audio** | CoreAudio | Windows.Devices.Audio | wpctl (PipeWire) / pactl |
| **App Automation** | AppleScript / JXA | COM Objects (PowerShell) | D-Bus (gdbus/qdbus) |
| **Clipboard** | pbcopy/pbpaste | clip.exe / PowerShell | xclip / wl-copy |

---

## 2. Windows Complete Control

### 2.1 UI Automation (UIA) — The Primary Perception Layer

Windows UI Automation is the direct equivalent of macOS AXUIElement. It exposes every UI element as an `AutomationElement` in a tree structure rooted at the desktop.

**Architecture:**
```
Desktop (root)
 └── Application Window
      ├── Menu Bar
      │    └── Menu Items (Invoke, ExpandCollapse patterns)
      ├── Toolbar
      │    └── Buttons (Invoke pattern)
      ├── Content Area
      │    ├── Text Fields (Value, Text patterns)
      │    ├── Lists (Selection, ScrollItem patterns)
      │    └── Data Grids (Grid, Table patterns)
      └── Status Bar
```

**Key Properties (analogous to macOS AX attributes):**
- `AutomationId` — stable programmatic identifier (like AXIdentifier)
- `Name` — human-readable label (like AXTitle)
- `ClassName` — Win32 class name
- `ControlType` — 38 defined types (Button, CheckBox, ComboBox, DataGrid, etc.)
- `BoundingRectangle` — screen coordinates (like AXPosition + AXSize)
- `IsEnabled`, `IsOffscreen`, `HasKeyboardFocus`

**Control Patterns (analogous to macOS AX Actions):**
| Pattern | Purpose | macOS Equivalent |
|---|---|---|
| InvokePattern | Click buttons | AXPress |
| ValuePattern | Get/set text fields | AXValue (get/set) |
| TogglePattern | Check/uncheck | AXPress on checkboxes |
| SelectionPattern | Select items in lists | AXSelected |
| ExpandCollapsePattern | Open/close dropdowns | AXOpen/AXPress |
| ScrollPattern | Scroll containers | AXScroll |
| WindowPattern | Move/resize/close windows | AXRaise/AXMove |
| TextPattern | Rich text manipulation | N/A (macOS has no equivalent) |
| TransformPattern | Rotate/resize elements | N/A |
| GridPattern | Navigate grid cells | N/A |

**Tree Views:**
UIA provides three views of the same tree:
- **Raw View** — everything, including internal framework elements
- **Control View** — only interactive/informational controls (equivalent to filtering only interactive roles in macOS)
- **Content View** — only meaningful content elements

Recommendation: Use **Control View** for computer-pilot snapshots (matches macOS behavior of filtering to interactive elements only).

**Framework Coverage:**
| UI Framework | UIA Support | Notes |
|---|---|---|
| WPF | Excellent | Best UIA support, full pattern coverage |
| WinForms | Good | Some custom controls need AutomationPeer |
| UWP / WinUI | Excellent | Native UIA support |
| Win32 (native) | Good | Older MSAA bridge, but functional |
| Qt | Moderate | Qt5+ has UIA provider, Qt4 needs MSAA |
| Electron/CEF | Good | Chromium exposes UIA tree |
| Java Swing | Poor | Needs Java Access Bridge enabled |

**Performance Considerations:**
- UIA calls are cross-process COM IPC (similar to macOS AX's Mach IPC)
- Batch operations via `CacheRequest` (like macOS `AXUIElementCopyMultipleAttributeValues`)
- Full desktop tree traversal can be slow; scope to specific windows
- Set `TreeScope.Subtree` with conditions to limit traversal depth

### 2.2 Win32 API — What UIA Can't Do

Win32 provides capabilities that complement UIA:

| Capability | API | Why UIA Can't |
|---|---|---|
| Low-level mouse/keyboard | `SendInput()` | UIA has InvokePattern but no direct mouse coordinates |
| Send messages to background windows | `SendMessage()` / `PostMessage()` | UIA can only act on visible, focused elements |
| Window enumeration by class | `FindWindow()` / `EnumWindows()` | UIA window discovery is slower |
| Process/thread info | `GetWindowThreadProcessId()` | UIA provides ProcessId but less detail |
| Hook events globally | `SetWindowsHookEx()` | UIA events are scoped, not global hooks |
| DPI/monitor awareness | `GetDpiForWindow()` | Needed for coordinate translation |
| Screenshot via GDI | `BitBlt()` / DXGI duplication | UIA has no screenshot capability |

**Key Insight:** The relationship between Win32 and UIA on Windows mirrors CGEvent vs AXUIElement on macOS. UIA provides semantic actions (click this button by name), Win32/SendInput provides coordinate-level fallback. computer-pilot should use both in the same 15-step chain pattern.

### 2.3 Windows Input Synthesis — The Action Layer

```
Tier 1: UIA Patterns (InvokePattern.Invoke(), ValuePattern.SetValue())
  ↓ when pattern not supported
Tier 2: UIA SetFocus() + SendInput() keyboard shortcut
  ↓ when element can't be focused
Tier 3: SendInput() mouse click at BoundingRectangle coordinates
```

`SendInput()` is the Windows equivalent of macOS `CGEventCreateMouseEvent`:
- Injects keyboard and mouse events at the OS input queue level
- Subject to UIPI (User Interface Privilege Isolation) — cannot inject into higher-integrity processes
- Works with coordinate-based automation when UIA patterns fail
- Supports both physical key codes and Unicode text injection

### 2.4 COM Automation — Windows-Specific Superpower

COM (Component Object Model) lets you programmatically control applications that expose COM interfaces. This has no direct macOS equivalent (AppleScript/JXA is the closest analog).

```powershell
# Excel automation
$excel = New-Object -ComObject Excel.Application
$excel.Visible = $true
$wb = $excel.Workbooks.Open("C:\data.xlsx")
$ws = $wb.Sheets.Item(1)
$ws.Range("A1").Value2 = "Hello"
$wb.Save()
$excel.Quit()

# Word automation
$word = New-Object -ComObject Word.Application
$doc = $word.Documents.Add()
$doc.Content.Text = "Generated by computer-pilot"
$doc.SaveAs("C:\output.docx")

# Outlook automation
$outlook = New-Object -ComObject Outlook.Application
$mail = $outlook.CreateItem(0)
$mail.To = "user@example.com"
$mail.Subject = "Automated email"
$mail.Body = "Sent by computer-pilot"
$mail.Send()
```

**COM-Automatable Applications:**
- Microsoft Office (Excel, Word, Outlook, PowerPoint, Access)
- Internet Explorer (legacy)
- Windows Shell (Shell.Application — file operations, dialogs)
- Windows Script Host (WScript.Shell — run commands, registry, shortcuts)
- ADODB (database access)
- WMI (system management — see below)

### 2.5 WMI / CIM — System Introspection

Windows Management Instrumentation provides deep system introspection:

```powershell
# Process information
Get-CimInstance Win32_Process | Select Name, ProcessId, WorkingSetSize

# Hardware info
Get-CimInstance Win32_ComputerSystem
Get-CimInstance Win32_OperatingSystem

# Service management
Get-CimInstance Win32_Service | Where Status -eq "Running"

# Registry (via StdRegProv)
# Read, write, create, delete registry keys and values

# Event monitoring
Register-CimIndicationEvent -ClassName Win32_ProcessStartTrace -Action { ... }
```

**Key CIM/WMI Classes:** Win32_Process, Win32_Service, Win32_OperatingSystem, Win32_NetworkAdapterConfiguration, Win32_LogicalDisk, Win32_UserAccount, StdRegProv (registry).

Note: WMI cmdlets (`Get-WmiObject`) are deprecated in PowerShell 6+. Use CIM cmdlets (`Get-CimInstance`) instead.

### 2.6 Registry Manipulation

The Windows Registry is the central configuration store. computer-pilot can read/write settings:

```powershell
# Read
Get-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer"

# Write
Set-ItemProperty -Path "HKCU:\..." -Name "PropertyName" -Value "Value"

# Create key
New-Item -Path "HKLM:\SOFTWARE\ComputerPilot"

# Remote registry (via CIM)
Invoke-CimMethod -ClassName StdRegProv -MethodName GetStringValue -Arguments @{...} -ComputerName "REMOTE-PC"
```

### 2.7 ms-settings: URI Scheme

Windows Settings pages can be opened programmatically via URI schemes:

```powershell
Start-Process "ms-settings:display"           # Display settings
Start-Process "ms-settings:network-wifi"       # WiFi settings
Start-Process "ms-settings:privacy"            # Privacy settings
Start-Process "ms-settings:windowsupdate"      # Windows Update
Start-Process "ms-settings:bluetooth"          # Bluetooth
Start-Process "ms-settings:sound"              # Sound settings
Start-Process "ms-settings:apps-volume"        # App volume
Start-Process "ms-settings:defaultapps"        # Default apps
Start-Process "ms-settings:personalization"    # Personalization
```

Approximately 200+ URI commands available. Can be discovered programmatically by scanning `SystemSettings.dll` with a PowerShell script.

### 2.8 Recommended Windows Native Helper

Analogous to the macOS Swift helper, Windows needs a native helper:

```
cu (TypeScript CLI, npm distribution)
 │
 │  child_process.spawn (JSON over stdin/stdout)
 ▼
desktop-helper-win.exe (C# / .NET native binary, pre-built x64 + arm64)
 ├── UIA (UIAutomationClient)  → read automation tree, perform UIA actions
 ├── SendInput (Win32 P/Invoke) → synthesize mouse/keyboard events
 ├── Windows.Media.OCR          → built-in OCR (like macOS Vision Framework)
 └── DXGI Desktop Duplication   → screenshots
```

**Why C# / .NET:**
- UIAutomationClient is a .NET assembly; C# calls it natively without FFI
- FlaUI (popular open-source) wraps it with additional convenience
- .NET Native AOT or `PublishSingleFile` produces a single .exe
- COM automation also natural from C#
- PowerShell integration trivial from C#

**Alternative: Rust + windows-rs crate** — would produce smaller binaries and no .NET dependency, but UIA bindings are more verbose. C# is pragmatically better for Windows automation.

---

## 3. Linux Complete Control

### 3.1 The Fragmentation Problem

Linux desktop automation faces fundamental fragmentation at three levels:

```
Level 1: Display Server         X11  vs  Wayland
Level 2: Compositor             GNOME/Mutter  vs  KDE/KWin  vs  Sway  vs  Hyprland  vs  ...
Level 3: Desktop Environment    GNOME  vs  KDE  vs  XFCE  vs  MATE  vs  ...
Level 4: Toolkit                GTK  vs  Qt  vs  Electron  vs  ...
```

This is the single biggest challenge for cross-platform computer control.

### 3.2 AT-SPI2 — The Universal(ish) Accessibility Layer

AT-SPI2 (Assistive Technology Service Provider Interface) is the Linux equivalent of macOS AXUIElement and Windows UIA. It runs over D-Bus and provides:

**Architecture:**
```
Application (GTK/Qt)
 ├── Exposes AT-SPI2 interface via D-Bus
 └── Registers with AT-SPI2 registryd daemon

AT-SPI2 Registryd
 ├── Tracks all accessible applications
 ├── Runs on separate D-Bus (not session bus)
 └── Mediates between applications and clients

Client (computer-pilot)
 ├── Connects to AT-SPI2 bus
 ├── Discovers applications
 ├── Reads element trees
 └── Performs actions
```

**Capabilities:**
| Feature | AT-SPI2 Support | Notes |
|---|---|---|
| Element tree traversal | Yes | Similar to macOS AX tree DFS |
| Role identification | Yes | ~80 roles (Button, CheckBox, TextField, etc.) |
| Properties (name, description, value) | Yes | Comparable to AXTitle, AXValue |
| Bounds/position | Yes | Screen coordinates |
| Actions (click, activate) | Yes | DoAction interface |
| Text manipulation | Yes | Full text editing API |
| Selection | Yes | For lists, trees, etc. |
| Table navigation | Yes | Row/column access |
| State queries | Yes | Focused, checked, enabled, visible |

**Framework Coverage:**
| UI Framework | AT-SPI2 Support | Notes |
|---|---|---|
| GTK 3/4 | Excellent | Native AT-SPI2 provider |
| Qt 5/6 | Good | QAccessible → AT-SPI2 bridge |
| Electron/Chromium | Good | Chromium's accessibility layer |
| Java Swing | Moderate | Java Access Bridge |
| wxWidgets | Moderate | Via ATK/AT-SPI2 |
| SDL/OpenGL/Vulkan games | None | No accessibility tree |

**Key Limitation:** AT-SPI2 works on both X11 and Wayland, making it the best universal approach for semantic element interaction on Linux.

### 3.3 X11 Input Injection (xdotool & friends)

X11 provides the richest input injection and window management:

```bash
# Mouse
xdotool mousemove 500 300           # move cursor
xdotool click 1                      # left click
xdotool click --window $WID 1       # click in specific window

# Keyboard
xdotool key ctrl+c                   # key combo
xdotool type "hello world"          # type text

# Window management
xdotool search --name "Firefox"      # find window
xdotool windowactivate $WID          # focus window
xdotool windowmove $WID 100 100     # move window
xdotool windowsize $WID 800 600     # resize window
xdotool windowminimize $WID          # minimize

# Additional tools
wmctrl -l                            # list windows
wmctrl -a "Firefox"                  # activate window
xprop -id $WID                       # window properties
xclip -selection clipboard -o        # read clipboard
```

**X11 Advantages:**
- Complete window management (move, resize, minimize, maximize, close)
- Click in background windows (without focus)
- Get/set window properties
- Window stacking order control
- Multiple monitor coordinate handling

### 3.4 Wayland Input Injection — The Hard Part

Wayland deliberately removed X11's ability for clients to spy on or inject input into other clients (security by design). This creates significant challenges:

**ydotool (uinput-based):**
```bash
ydotool mousemove --absolute -x 500 -y 300   # move cursor
ydotool click 0xC0                            # left click
ydotool type "hello world"                    # type text
ydotool key ctrl+c                            # key combo
```

Limitations:
- Requires root or uinput group membership
- Runs as a daemon (ydotoold)
- NO window management (cannot move, resize, list, focus windows)
- NO ability to target specific windows
- Purely blind input injection at kernel level

**Compositor-Specific Solutions:**
| Compositor | Window Management Tool | Notes |
|---|---|---|
| KDE/KWin | kdotool | Generates KWin scripts on-the-fly, xdotool-like API |
| wlroots-based (Sway, Hyprland) | wlrctl | IPC to compositor for window/output control |
| GNOME/Mutter | gdbus + GNOME Shell JS | Via org.gnome.Shell.Eval (security restricted) |
| Hyprland | hyprctl | JSON IPC, full window management |

**The Strategy:**
```
Input Injection:           ydotool (universal, works everywhere)
Window Management:         Compositor-specific (kdotool / wlrctl / hyprctl / gdbus)
Element Interaction:       AT-SPI2 (universal, works everywhere)
```

### 3.5 Linux Desktop Environment Automation

**GNOME:**
```bash
# Settings
gsettings set org.gnome.desktop.interface gtk-theme "Adwaita-dark"
gsettings set org.gnome.desktop.background picture-uri "file:///path/to/wallpaper.jpg"

# D-Bus control
gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
  --method org.gnome.Shell.Eval "global.get_window_actors().length"

# Notifications
gdbus call --session --dest org.freedesktop.Notifications \
  --object-path /org/freedesktop/Notifications \
  --method org.freedesktop.Notifications.Notify ...
```

**KDE:**
```bash
# Settings
kwriteconfig5 --file kwinrc --group Windows --key BorderlessMaximizedWindows true
kwriteconfig6 --file kdeglobals --group General --key ColorScheme "BreezeDark"

# D-Bus control
qdbus org.kde.KWin /KWin org.kde.KWin.reconfigure
qdbus org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript "..."

# Window management via KWin scripting
kdotool search --name "Firefox" windowactivate
```

**Key Insight:** DE-specific commands should be exposed as platform extensions (`cu linux:gnome:gsettings`, `cu linux:kde:kwriteconfig`) while core commands (`cu click`, `cu snapshot`) use the universal AT-SPI2 + ydotool path.

### 3.6 Linux System Control

```bash
# Service management
systemctl status nginx
systemctl start/stop/restart nginx
systemctl enable/disable nginx

# Network
nmcli device status
nmcli connection show
nmcli device wifi list
nmcli connection up "MyWiFi"

# Audio (PipeWire/PulseAudio)
wpctl status                          # PipeWire
wpctl set-volume @DEFAULT_SINK@ 50%  # Set volume
pactl list sinks                      # PulseAudio
pactl set-sink-volume 0 50%

# Package management (distro-specific)
apt install/remove/update              # Debian/Ubuntu
dnf install/remove/update              # Fedora/RHEL
pacman -S/-R/-Syu                     # Arch

# Scheduling
crontab -e                             # Cron jobs
at now + 5 minutes                     # One-time scheduling
systemd-run --on-calendar="*-*-* 09:00" ...  # systemd timer
```

### 3.7 Recommended Linux Native Helper

```
cu (TypeScript CLI, npm distribution)
 │
 │  child_process.spawn (JSON over stdin/stdout)
 ▼
desktop-helper-linux (Rust binary, pre-built x86_64 + arm64)
 ├── AT-SPI2 (via atspi crate/D-Bus)  → read accessibility tree, perform actions
 ├── ydotool (subprocess or uinput)    → synthesize mouse/keyboard events
 ├── X11/XShm or grim/slurp           → screenshots (X11 or Wayland)
 └── Tesseract (optional, via CLI)     → OCR fallback
```

**Why Rust for Linux:**
- No runtime dependency (.NET would be unusual on Linux)
- Excellent D-Bus crates (zbus) for AT-SPI2 communication
- Cross-compiles easily for x86_64 and arm64
- Single static binary, no library dependencies
- The atspi crate provides typed AT-SPI2 bindings

**Why not Swift on Linux:**
- Swift on Linux lacks Foundation frameworks (no Vision, no ScreenCaptureKit)
- The Rust ecosystem for Linux system programming is far more mature

### 3.8 Handling Display Server Detection

```
detect display server:
  if $XDG_SESSION_TYPE == "x11"  →  use xdotool + X11 screenshots
  if $XDG_SESSION_TYPE == "wayland":
    detect compositor:
      if $XDG_CURRENT_DESKTOP contains "KDE"     →  use kdotool for window mgmt
      if $XDG_CURRENT_DESKTOP contains "GNOME"   →  use gdbus for window mgmt
      if $XDG_CURRENT_DESKTOP contains "sway"    →  use wlrctl / swaymsg
      if $HYPRLAND_INSTANCE_SIGNATURE set         →  use hyprctl
      else                                        →  ydotool only (no window mgmt)
    
    input injection: always ydotool (universal)
    accessibility: always AT-SPI2 (universal)
```

---

## 4. Cross-Platform Architecture Decision

### Evaluating the Four Approaches

Your existing research doc identified four approaches. Here is the analysis with full cross-platform research:

### Approach A: Pure Abstraction Layer
```
cu.click(element)  →  macOS: AXPress  |  Windows: InvokePattern  |  Linux: AT-SPI DoAction
```
**Verdict: Necessary but insufficient.** The core observe-act loop (snapshot, click, type, key, screenshot) MUST have a unified API. But limiting to only this throws away 80% of each platform's power.

### Approach B: Platform-Specific Commands with Shared Interface
```
cu click 3                    # Works everywhere (unified)
cu macos:applescript "..."    # macOS only
cu win:powershell "..."       # Windows only  
cu linux:gsettings "..."      # Linux only
```
**Verdict: The winning approach.** This is what computer-pilot should do.

### Approach C: Capability-Based Discovery
```
cu capabilities               # Reports what this platform can do
cu capabilities --json        # Machine-readable for agents
```
**Verdict: Essential complement to Approach B.** Agents need to know what's available.

### Approach D: Semantic Actions
```
cu do "open file X"           # High-level intent
cu do "install app Z"         # Platform figures out implementation
```
**Verdict: Future layer on top.** Too large an implementation surface for MVP, but the right long-term direction.

### Recommended Architecture: B + C (with A as foundation)

```
                    ┌──────────────────────────────────────┐
                    │          cu (TypeScript CLI)          │
                    │   npm install -g computer-pilot       │
                    │                                      │
                    │  ┌────────────────────────────────┐  │
                    │  │     Unified Command Layer       │  │
                    │  │  cu apps/snapshot/click/type/   │  │
                    │  │  key/screenshot/scroll/drag     │  │
                    │  │  cu capabilities                │  │
                    │  └──────────┬─────────────────────┘  │
                    │             │                         │
                    │  ┌──────────┴─────────────────────┐  │
                    │  │   Platform Extension Layer      │  │
                    │  │  cu macos:applescript "..."     │  │
                    │  │  cu win:powershell "..."        │  │
                    │  │  cu win:registry "..."          │  │
                    │  │  cu linux:gsettings "..."       │  │
                    │  │  cu linux:systemctl "..."       │  │
                    │  └──────────┬─────────────────────┘  │
                    └─────────────┼─────────────────────────┘
                                  │
                    ┌─────────────┼─────────────────────────┐
                    │    Platform Detection + Dispatch       │
                    │    process.platform → spawn helper     │
                    └──────┬──────────┬──────────┬──────────┘
                           │          │          │
              ┌────────────┴──┐ ┌─────┴────────┐ ┌┴──────────────┐
              │ macOS Helper  │ │ Windows Helper│ │ Linux Helper  │
              │ (Swift)       │ │ (C# .NET AOT) │ │ (Rust)        │
              │               │ │               │ │               │
              │ AXUIElement   │ │ UIA           │ │ AT-SPI2/D-Bus │
              │ CGEvent       │ │ SendInput     │ │ ydotool/xdotool│
              │ ScreenCapture │ │ DXGI/GDI      │ │ X11/grim      │
              │ Vision OCR    │ │ Windows OCR   │ │ Tesseract     │
              └───────────────┘ └───────────────┘ └───────────────┘
```

### Unified Command Mapping

| Command | macOS | Windows | Linux |
|---|---|---|---|
| `cu apps` | NSWorkspace | UIA root children | AT-SPI2 registry |
| `cu snapshot "App"` | AXUIElement DFS, Control View | UIA TreeWalker, Control View | AT-SPI2 tree traversal |
| `cu click <ref>` | 15-step AX chain → CGEvent | UIA Pattern chain → SendInput | AT-SPI2 DoAction → ydotool |
| `cu click <x,y>` | CGEvent mouse click | SendInput mouse click | xdotool/ydotool click |
| `cu type <ref> "text"` | AXSetValue → CGEvent keys | ValuePattern.SetValue → SendInput | AT-SPI2 SetText → ydotool |
| `cu type "text"` | CGEvent keyboard | SendInput keyboard | xdotool/ydotool type |
| `cu key <combo>` | CGEvent key combo | SendInput key combo | xdotool/ydotool key |
| `cu screenshot` | ScreenCaptureKit | DXGI / BitBlt | X11+XShm / grim |
| `cu ocr` | Vision Framework | Windows.Media.OCR | Tesseract CLI |
| `cu scroll <dir>` | CGEvent scrollWheel | SendInput wheel | xdotool/ydotool scroll |
| `cu focus "App"` | AXRaise | UIA SetFocus + SetForegroundWindow | wmctrl/kdotool/wlrctl |
| `cu windows` | CGWindowListCopyWindowInfo | EnumWindows + UIA | wmctrl / compositor IPC |
| `cu capabilities` | Returns macOS capability list | Returns Windows capability list | Returns Linux capability list |

### Platform Extension Commands

**macOS-specific:**
```bash
cu macos:applescript 'tell app "Finder" to activate'
cu macos:jxa "Application('Finder').activate()"
cu macos:defaults read com.apple.dock orientation
cu macos:open "https://example.com"              # open command
```

**Windows-specific:**
```bash
cu win:powershell "Get-Process | Select Name"
cu win:registry read "HKCU\Software\..."
cu win:registry write "HKCU\..." "key" "value"
cu win:com "Excel.Application" "Workbooks.Open('data.xlsx')"
cu win:settings "display"                         # ms-settings:display
cu win:wmi "Win32_Process" "Name,ProcessId"
```

**Linux-specific:**
```bash
cu linux:gsettings get org.gnome.desktop.interface gtk-theme
cu linux:gsettings set org.gnome.desktop.interface gtk-theme "Adwaita-dark"
cu linux:systemctl status nginx
cu linux:nmcli device status
cu linux:dbus call org.freedesktop.Notifications ...
```

### The `cu capabilities` Command

Critical for agent-adaptive behavior:

```json
{
  "platform": "linux",
  "display_server": "wayland",
  "compositor": "KDE/KWin",
  "desktop_environment": "KDE Plasma 6",
  "capabilities": {
    "accessibility_tree": true,
    "accessibility_backend": "AT-SPI2",
    "coordinate_input": true,
    "input_backend": "ydotool",
    "window_management": true,
    "window_backend": "kdotool",
    "screenshot": true,
    "screenshot_backend": "grim",
    "ocr": true,
    "ocr_backend": "tesseract",
    "clipboard": true,
    "clipboard_backend": "wl-copy"
  },
  "extensions": [
    "linux:gsettings",
    "linux:kwriteconfig",
    "linux:qdbus",
    "linux:systemctl",
    "linux:nmcli",
    "linux:wpctl"
  ],
  "limitations": [
    "Wayland: cannot send input to background windows",
    "Wayland: window management requires compositor-specific tool"
  ]
}
```

---

## 5. Mobile Platforms

### 5.1 Android — Feasible and Valuable

Android automation is well-supported via ADB:

**Capabilities:**
| Feature | Tool | Notes |
|---|---|---|
| Screenshots | `adb shell screencap` | PNG capture, ~200ms |
| Input injection | `adb shell input tap/swipe/text/keyevent` | Full touch/keyboard |
| UI tree | `adb shell uiautomator dump` | XML accessibility tree |
| App management | `adb shell am/pm` | Start/stop/install apps |
| File transfer | `adb push/pull` | Full filesystem access |
| Screen recording | `adb shell screenrecord` | Video capture |
| Shell access | `adb shell` | Full Linux shell |
| Network config | `adb shell settings` | WiFi, mobile data |

**MCP Integration (already exists):**
The xuegao-tzx Android ADB MCP server (released March 2026) provides 41 tools covering screenshots, OCR, UI inspection, touch automation, app management, file transfer, and device settings.

**Recommendation:** Android is a strong candidate for computer-pilot Phase 6. The architecture fits perfectly:
```
cu (TypeScript CLI)
 │
 │  child_process.spawn
 ▼
adb (Android Debug Bridge)
 ├── uiautomator dump    → accessibility tree (like AT-SPI2/UIA)
 ├── input tap/swipe     → touch events (like CGEvent/SendInput)
 ├── screencap           → screenshots
 └── shell               → system control
```

The same unified commands work:
```bash
cu --device android snapshot "Settings"    # UI automator dump
cu --device android click 3                # tap on element ref
cu --device android type 5 "hello"         # input text
cu --device android screenshot             # screencap
```

### 5.2 iOS — Severely Limited

iOS automation is restricted to Apple's controlled environments:

| Approach | Capabilities | Limitations |
|---|---|---|
| XCTest / XCUITest | Full UI automation | Requires Xcode, runs in test context only |
| Accessibility API | Element tree, actions | Only from within-app or test runner |
| Appium + WebDriverAgent | Remote control via USB | Complex setup, must be developer-signed |
| Apple Shortcuts | Task automation | Limited to Apple's predefined actions |

**Key Constraint:** iOS has no equivalent of ADB. You cannot inject input, capture screenshots, or read the UI tree from an external process without Xcode/developer tooling. Apple's security model makes this fundamentally different from Android.

**Recommendation:** iOS is a stretch goal. If pursued, the path is Appium + WebDriverAgent running on a Mac connected via USB. This is fragile and requires iOS developer setup. Not recommended for MVP or even Phase 6.

### 5.3 Mobile Strategy Summary

| Platform | Priority | Effort | Value | Recommendation |
|---|---|---|---|---|
| Android | Medium | Low (ADB is well-documented) | High (3B+ devices) | Phase 6 extension |
| iOS | Low | Very High (Apple restrictions) | Medium | Defer indefinitely |

---

## 6. Remote/Cloud Control

### 6.1 Why Remote Matters

Running computer-pilot on a remote machine enables:
- CI/CD: automated UI testing in cloud VMs
- Security: sandbox agent actions in disposable VMs
- Scale: run multiple agents on cloud desktops simultaneously
- Cross-platform testing from any host

### 6.2 Cloud Desktop Providers

| Provider | Platform | Mechanism | Latency | Cost | computer-pilot Fit |
|---|---|---|---|---|---|
| E2B Desktop Sandbox | Linux | Docker + VNC/noVNC | <200ms spin-up | Pay-per-second | Excellent — designed for agents |
| Cua (trycua) | macOS/Linux/Windows | VM sandbox + SDK | Varies | Open source | Excellent — purpose-built for CUA |
| AWS WorkSpaces | Windows/Linux | RDP/PCoIP | ~50ms | $21+/mo | Enterprise use |
| Azure Virtual Desktop | Windows | RDP | ~30ms | Pay-per-use | Enterprise use |
| Google Cloud Workstations | Linux | SSH + browser | ~40ms | Pay-per-use | Developer use |
| Hetzner/Vultr VPS | Linux | SSH + VNC | ~20ms | $5+/mo | Budget option |

### 6.3 Remote Control Architecture

```
Local Machine                          Remote Machine
┌──────────────┐                      ┌──────────────────────┐
│ cu CLI       │                      │ cu-remote-agent      │
│              │  ── SSH/WebSocket ──→│                      │
│ cu --remote  │                      │ desktop-helper       │
│   user@host  │  ←─ JSON results ── │ (Swift/C#/Rust)      │
│              │                      │                      │
└──────────────┘                      │ Display: Xvfb/VNC    │
                                      └──────────────────────┘
```

Two modes:
1. **SSH tunnel**: `cu --remote ssh://user@host` — spawn remote helper via SSH, pipe JSON
2. **Agent mode**: `cu-agent` runs as daemon on remote, exposes WebSocket/REST API

### 6.4 E2B Integration (Recommended Cloud Path)

```typescript
import { Sandbox } from '@e2b/desktop';

// Spin up a cloud desktop in <200ms
const sandbox = await Sandbox.create();

// computer-pilot commands run inside the sandbox
await sandbox.commands.run('cu snapshot "Firefox"');
await sandbox.commands.run('cu click 3');

// Get screenshot
const screenshot = await sandbox.screenshot();

// Tear down
await sandbox.kill();
```

**Recommendation:** E2B is the ideal cloud backend. It provides:
- Linux desktop sandboxes designed for AI agent computer use
- <200ms startup time
- Python and JavaScript SDKs
- Already used by ~50% of Fortune 500 for AI workloads

---

## 7. Implementation Roadmap

### Phase 1: macOS MVP (Current)
Already designed in research.md. 6 commands, Swift helper, AX-first perception.

### Phase 2: Windows Backend (High Priority)
**Effort: 4-6 weeks**

| Task | Details |
|---|---|
| C# native helper | .NET 8 AOT single-file binary |
| UIA tree reading | UIAutomationClient, Control View, batch CacheRequest |
| UIA action chain | InvokePattern → ValuePattern → TogglePattern → SendInput fallback |
| Screenshots | DXGI Desktop Duplication (hardware-accelerated) |
| OCR | Windows.Media.OCR (built-in, like macOS Vision) |
| Platform detection | TypeScript: `process.platform === 'win32'` → spawn helper-win.exe |
| Win32 coordinate fallback | SendInput for mouse/keyboard when UIA patterns fail |
| npm bundling | Pre-built .exe in npm package (like macOS Swift binary) |

**Windows-specific challenges:**
- DPI scaling: must handle per-monitor DPI awareness
- UAC: cannot automate elevated windows from non-elevated process
- Antivirus: SendInput may trigger false positives
- .NET AOT: currently requires MSVC build tools

### Phase 3: Linux Backend (Medium Priority)
**Effort: 4-6 weeks**

| Task | Details |
|---|---|
| Rust native helper | Single static binary via musl |
| AT-SPI2 tree reading | zbus + atspi crate, D-Bus connection |
| AT-SPI2 actions | DoAction interface for semantic clicks |
| Display server detection | `$XDG_SESSION_TYPE`, `$XDG_CURRENT_DESKTOP` |
| X11 input (xdotool path) | Via subprocess or libX11 bindings |
| Wayland input (ydotool path) | Via subprocess to ydotoold |
| Screenshots | X11: XShm / Wayland: grim (compositor-agnostic screenshotter) |
| OCR | Tesseract CLI fallback (not built-in like macOS/Windows) |
| Window management | X11: wmctrl / Wayland: compositor-specific detection |

**Linux-specific challenges:**
- ydotool requires root or uinput group
- No built-in OCR (Tesseract must be installed separately)
- Wayland window management is compositor-specific
- AT-SPI2 may not be running by default on minimal installations
- grim requires slurp for region selection

### Phase 4: Platform Extensions
**Effort: 2-3 weeks per platform**

| Platform | Extensions |
|---|---|
| macOS | `cu macos:applescript`, `cu macos:defaults`, `cu macos:open` |
| Windows | `cu win:powershell`, `cu win:registry`, `cu win:com`, `cu win:settings` |
| Linux | `cu linux:gsettings`, `cu linux:systemctl`, `cu linux:nmcli`, `cu linux:dbus` |

### Phase 5: Capabilities Discovery
**Effort: 1-2 weeks**

- `cu capabilities` command
- JSON output describing available backends, features, limitations
- Agent-consumable format for adaptive behavior
- Platform-specific feature flags

### Phase 6: Mobile + Remote
**Effort: 3-4 weeks each**

- Android via ADB (`cu --device android`)
- Remote via SSH/WebSocket (`cu --remote ssh://...`)
- E2B cloud sandbox integration

### Priority & Effort Summary

```
Phase 1: macOS MVP           ████████████  (current, ~6 weeks)
Phase 2: Windows Backend     ████████████  (highest ROI, ~5 weeks)
Phase 3: Linux Backend       ████████████  (complex, ~5 weeks)
Phase 4: Platform Extensions ██████        (~3 weeks)
Phase 5: Capabilities API    ████          (~2 weeks)
Phase 6: Mobile + Remote     ████████      (~4 weeks)
                             ─────────────────────────────
                             Total: ~25 weeks for full cross-platform
```

---

## Appendix A: Lessons from Existing Cross-Platform Tools

### nut.js
- **What worked:** Single API for mouse/keyboard/screen across platforms. N-API native bindings for performance.
- **What didn't:** Linux X11-only (no Wayland). Accessibility tree support came late. Plugin architecture adds complexity.
- **Lesson for computer-pilot:** Use nut.js's API surface as inspiration but implement platform-specific helpers for deeper control.

### pyautogui
- **What worked:** Dead simple API. Screenshot + locate on screen. Cross-platform.
- **What didn't:** No accessibility tree. No window management. Coordinate-only = brittle.
- **Lesson:** Coordinate-only automation is a dead end. AX tree / UIA / AT-SPI2 must be primary.

### Robot Framework
- **What worked:** Keyword-driven testing. Cross-platform via libraries. Excellent reporting.
- **What didn't:** Not designed for real-time agent control. Setup is heavy.
- **Lesson:** Keep the CLI lightweight. Don't build a framework, build a tool.

### Sikuli
- **What worked:** Image recognition for any GUI. Platform-agnostic.
- **What didn't:** Constant image maintenance when UIs change. Slow. JVM dependency.
- **Lesson:** Vision-based matching is a fallback, not a primary strategy. Use accessibility trees first.

### usecomputer (Zig)
- **What worked:** True cross-platform from single codebase (Zig's @cImport). Minimal binary.
- **What didn't:** No accessibility tree = purely coordinate-based. Limited actions.
- **Lesson:** Zig's cross-compilation is impressive but the tool is too thin without AX support.

### Key Takeaway
Every cross-platform tool that succeeded prioritized ONE platform deeply first, then expanded. Tools that tried to be cross-platform from day one ended up with the lowest common denominator problem. computer-pilot's macOS-first approach is correct.

---

## Appendix B: Perception Strategy Comparison

Research from 2025-2026 clearly shows a hybrid approach wins:

| Approach | Speed | Accuracy | Token Cost | Coverage |
|---|---|---|---|---|
| Screenshot → Vision Model | 2-10s | ~72% (OSWorld) | ~1400 tokens/image | Universal |
| Accessibility Tree (text) | 50-300ms | ~85% (structured) | ~50-500 tokens | Framework-dependent |
| Screenshot → OCR (text) | 200-500ms | ~75% (text-heavy UIs) | ~200-800 tokens | Universal |
| Hybrid AX + Vision fallback | 50ms-3s | ~85%+ | Adaptive | Best overall |

**Screen2AX** (2025 paper) demonstrated that vision models can GENERATE accessibility tree metadata from screenshots, achieving 2.2x improvement over native AX on poorly-accessible apps. This validates computer-pilot's tiered approach:

```
Tier 1: Native AX/UIA/AT-SPI2 tree      (fastest, cheapest, most precise)
Tier 2: Screenshot + built-in OCR         (for apps with poor accessibility)
Tier 3: Screenshot as image to LLM        (last resort, universal but expensive)
```

---

## Appendix C: Security Considerations per Platform

| Concern | macOS | Windows | Linux |
|---|---|---|---|
| **Permission model** | Accessibility + Screen Recording + Input Monitoring | UAC elevation | Root/uinput group (Wayland) |
| **Isolation** | App Sandbox / TCC | UIPI (integrity levels) | None by default |
| **Anti-detection** | CGEvent is transparent | SendInput can be detected by anti-cheat | X11: transparent, Wayland: kernel-level |
| **Sandboxing** | Seatbelt profiles | App Container | Bubblewrap / Firejail |
| **Recommended** | Container + network isolation | Container + low-integrity process | Container + namespace isolation |

**Universal recommendation:** For untrusted agent actions, always use a VM or container sandbox. E2B, Cua, or Docker are the right answers. computer-pilot should support a `--sandbox` flag that routes all actions through an isolated environment.

---

## Sources

### Windows
- [UI Automation Overview - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-uiautomationoverview)
- [UI Automation Tree Overview - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-treeoverview)
- [UI Automation Specification - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/winauto/ui-automation-specification)
- [FlaUI - UI Testing Framework](https://www.thegreenreport.blog/articles/a-beginners-guide-to-using-flaui-for-windows-desktop-app-automation/a-beginners-guide-to-using-flaui-for-windows-desktop-app-automation.html)
- [FlaUI/FlaUI DeepWiki](https://deepwiki.com/FlaUI/FlaUI)
- [pywinauto GitHub](https://github.com/pywinauto/pywinauto)
- [SendInput - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)
- [WMI Registry Guide](https://copyprogramming.com/howto/wmi-tasks-registry)
- [WMI Start Page - Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/wmisdk/wmi-start-page)
- [ms-settings URI Complete List](https://pureinfotech.com/windows-11-mssettings-uri-commands/)
- [ms-settings Enterprise Guide - Microsoft](https://techcommunity.microsoft.com/blog/coreinfrastructureandsecurityblog/understanding-windows-settings-uris-and-how-to-use-them-in-enterprise-environmen/4481486)
- [PowerShell COM Objects Guide](https://www.computerperformance.co.uk/powershell/comobject/)
- [PowerShell Office Automation](https://automate.fortra.com/blog/interacting-ms-office-using-powershell)
- [WinAppDriver - BrowserStack](https://www.browserstack.com/guide/test-windows-desktop-app-using-appium-winappdriver)
- [Appium Windows Driver](https://github.com/appium/appium-windows-driver)
- [NodeRT Windows.UI.UIAutomation](https://www.npmjs.com/package/@nodert-win10-21h1/windows.ui.uiautomation)

### Linux
- [AT-SPI2 - freedesktop.org](https://www.freedesktop.org/wiki/Accessibility/AT-SPI2/)
- [AT-SPI on D-Bus - Linux Foundation](https://wiki.linuxfoundation.org/accessibility/atk/at-spi/at-spi_on_d-bus)
- [AT-SPI2 Wikipedia](https://en.wikipedia.org/wiki/Assistive_Technology_Service_Provider_Interface)
- [ydotool GitHub](https://github.com/ReimuNotMoe/ydotool)
- [kdotool GitHub](https://github.com/jinliu/kdotool)
- [wlrctl - Raspberry Pi Forums](https://forums.raspberrypi.com/viewtopic.php?t=371406)
- [Wayland Fragmentation Discussion - HN](https://news.ycombinator.com/item?id=45942109)
- [dogtail - GitLab](https://gitlab.com/dogtail/dogtail)
- [LDTP GitHub](https://github.com/ldtp/ldtp2)
- [D-Bus Control - Linux Journal](https://www.linuxjournal.com/article/10455)
- [qdbus Command Guide](https://commandmasters.com/commands/qdbus-common/)
- [PipeWire Guide](https://github.com/mikeroyal/PipeWire-Guide)
- [PipeWire ArchWiki](https://wiki.archlinux.org/title/PipeWire)
- [nmcli Reference Manual](https://networkmanager.dev/docs/api/latest/nmcli.html)

### Cross-Platform & AI Agents
- [nut.js](https://nutjs.dev/)
- [nut.js GitHub](https://github.com/nut-tree/nut.js/)
- [Claude Computer Use Tool Docs](https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool)
- [OpenAI CUA](https://openai.com/index/computer-using-agent/)
- [OpenAI Computer Use API](https://developers.openai.com/api/docs/guides/tools-computer-use)
- [Anthropic vs OpenAI CUA - WorkOS](https://workos.com/blog/anthropics-computer-use-versus-openais-computer-using-agent-cua)
- [UI-TARS Desktop - ByteDance](https://github.com/bytedance/UI-TARS-desktop)
- [Cua Framework](https://github.com/trycua/cua)
- [E2B Desktop Sandbox](https://github.com/e2b-dev/desktop)
- [E2B Documentation](https://e2b.dev/docs)
- [Screen2AX Paper](https://arxiv.org/abs/2507.16704)
- [DOM vs Screenshots for Agents - Medium](https://medium.com/@i_48340/how-ai-agents-actually-see-your-screen-dom-control-vs-screenshots-explained-dab80c2b31d7)
- [pyautogui Roadmap](https://pyautogui.readthedocs.io/en/latest/roadmap.html)

### Mobile
- [Android uiautomator2 Python](https://github.com/openatx/uiautomator2)
- [ADB MCP Server](https://www.pulsemcp.com/servers/xuegao-tzx-android-adb)
- [MobileAgent](https://github.com/X-PLUG/MobileAgent)
- [iOS Automated UI Testing Guide](https://testgrid.io/blog/guide-on-ios-automated-ui-testing/)
- [XCUITest Tutorial](https://www.lambdatest.com/xcuitest)

### Architecture & Patterns
- [2026 Guide to Agentic Workflow Architectures](https://www.stackai.com/blog/the-2026-guide-to-agentic-workflow-architectures)
- [AI Computer-Use Benchmarks Guide](https://o-mega.ai/articles/the-2025-2026-guide-to-ai-computer-use-benchmarks-and-top-ai-agents)
- [Agentic AI Design Patterns](https://research.aimultiple.com/agentic-ai-design-patterns/)
