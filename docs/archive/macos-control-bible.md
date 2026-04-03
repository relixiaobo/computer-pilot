# macOS Programmatic Control Bible

> Exhaustive reference for every way to programmatically control macOS.
> Researched on macOS 15.7.3 (Sequoia), Apple Silicon M3.

---

## Table of Contents

1. [AppleScript / JXA Scriptable Apps](#1-applescript--jxa-scriptable-apps)
2. [System Services & APIs](#2-macos-system-services--apis)
3. [Shortcuts / Automator](#3-macos-shortcuts--automator)
4. [Launch Services & App Management](#4-launch-services--app-management)
5. [File System Deep Control](#5-file-system-deep-control)
6. [Input Methods Beyond Basic Mouse/Keyboard](#6-input-methods-beyond-basic-mousekeyboard)
7. [Process & Window Management](#7-process--window-management)
8. [Inter-Process Communication](#8-inter-process-communication)
9. [Terminal / Shell Integration](#9-terminal--shell-integration)
10. [Media & Creative Tools](#10-media--creative-tools)
11. [Private / Undocumented APIs](#11-private--undocumented-apis)
12. [Complete Permissions Map](#12-complete-permissions-map)
13. [Gap Analysis: What No Tool Does Today](#13-gap-analysis)
14. [AppleScript vs AX Tree: When to Use Which](#14-applescript-vs-ax-tree)

---

## 1. AppleScript / JXA Scriptable Apps

### Apps with AppleScript Dictionaries (confirmed via .sdef files)

| App | Bundle | Dictionary Richness | Create | Read | Update | Delete |
|-----|--------|-------------------|--------|------|--------|--------|
| **Finder** | System | Very rich | folders, aliases | items, properties, selection | name, tags, comments, position | files, folders |
| **Safari** | System | Rich | tabs, windows | URL, title, source, tabs | URL, current tab | tabs, windows |
| **Mail** | System | Very rich | messages (compose + send) | mailboxes, messages, attachments | read/unread, flags, move | messages |
| **Calendar** | System | Rich | events, calendars | events, attendees, alarms | event properties | events, calendars |
| **Notes** | System | Moderate | notes, folders | notes (body as HTML), folders | name, body | notes, folders |
| **Reminders** | System | Moderate | reminders, lists | reminders, due dates | complete, properties | reminders, lists |
| **Music** | System | Rich | playlists | tracks, playlists, current track | play/pause/volume/shuffle | playlist tracks |
| **Photos** | System | Moderate (read-heavy) | albums | media items, metadata, albums | keywords, description, favorite | albums |
| **Messages** | System | Limited | send message | chats, messages, participants | - | - |
| **Contacts** | System | Rich | people, groups | all contact fields | any field | people, groups |
| **Shortcuts** | System | Limited | - | list shortcuts | - | - |
| **QuickTime Player** | System | Moderate | recordings | documents | - | - |
| **TV** | System | Moderate | - | library, playlists | - | - |
| **System Settings** | System | Minimal | - | panes | reveal pane | - |
| **Google Chrome** | Third-party | Rich | tabs, windows | URL, title, execute JS | URL, navigate | tabs, windows |
| **Arc** | Third-party | Has .sdef | TBD | TBD | TBD | TBD |
| **Ghostty** | Third-party | Has .sdef | TBD | TBD | TBD | TBD |

### System Events (the Universal Controller)

System Events is the single most powerful AppleScript target. It provides:

**UI Scripting (control ANY app, even non-scriptable ones):**
```applescript
tell application "System Events"
    tell process "AppName"
        click button "OK" of window 1
        click menu item "Copy" of menu "Edit" of menu bar 1
        set value of text field 1 of window 1 to "hello"
        keystroke "s" using {command down}
        key code 36  -- Return key
    end tell
end tell
```

**System-level operations:**
- Login items: `get name of every login item`, `make login item`, `delete login item`
- Dark mode: `dark mode of appearance preferences` (read/write)
- Processes: `name of every process`, `frontmost process`, process properties
- Disks: `name of every disk`, eject
- Screen saver: `start current screen saver`
- Property lists: read/write .plist files programmatically
- Folder actions: register scripts to run when folder contents change
- XML parsing: built-in XML element manipulation
- Aliases: create/resolve file aliases

**Permission required:** Accessibility (for UI Scripting), Automation (per-app-pair)

### JXA (JavaScript for Automation)

JXA has **full parity with AppleScript** plus critical advantages:

```javascript
// JXA via: osascript -l JavaScript -e '...'

// Everything AppleScript can do
var finder = Application("Finder");
finder.windows[0].name();

// PLUS: ObjC Bridge — call ANY native API
ObjC.import("AppKit");
ObjC.import("Foundation");
ObjC.import("CoreGraphics");

// Examples of ObjC bridge power:
$.NSWorkspace.sharedWorkspace.runningApplications;  // all apps
$.NSScreen.screens;  // all displays
$.NSPasteboard.generalPasteboard;  // clipboard with all types
$.NSFileManager.defaultManager;  // file operations
// Can call CoreLocation, AVFoundation, CoreBluetooth, etc.

// Native JSON support
JSON.stringify(data);
JSON.parse(text);

// Full regex
/pattern/g.test(string);

// Shell commands
var app = Application.currentApplication();
app.includeStandardAdditions = true;
app.doShellScript("ls -la");
```

**Key JXA advantages over AppleScript:**
1. ObjC bridge unlocks every Cocoa/CoreFoundation API without Swift compilation
2. Native JSON for structured data exchange with CLI tools
3. JavaScript regex, array methods, proper data structures
4. HTTP requests via NSURLSession through ObjC bridge
5. Same `osascript` binary, just `-l JavaScript` flag

**Key difference:** JXA was abandoned by Apple (no updates since ~2018). It works but has quirks with some newer APIs. Still fully functional on macOS 15.

### What AppleScript Can Do That AX Tree Cannot

| Capability | AppleScript | AX Tree |
|-----------|-------------|---------|
| App-specific semantic operations (send email, create event) | Yes | No |
| Execute JavaScript in Safari/Chrome | Yes | No |
| Get page source from browsers | Yes | No |
| Create/modify file tags, comments | Yes (Finder) | No |
| Play/pause/control music | Yes | Only via AXPress on buttons |
| Send iMessage | Yes | Would require navigating UI |
| Create calendar events with attendees | Yes | Would require navigating UI |
| Run shell commands (with admin privileges) | Yes | No |
| Read/write .plist files | Yes (System Events) | No |
| Manage login items | Yes | No |
| Toggle dark mode | Yes | No |
| Create native file/folder dialogs | Yes | No |

### What AX Tree Can Do That AppleScript Cannot

| Capability | AX Tree | AppleScript |
|-----------|---------|-------------|
| Access UI elements of non-scriptable apps | Yes | No (only UI Scripting via System Events) |
| Get exact pixel position/size of elements | Yes | Only via System Events |
| Read element state (enabled, focused, selected) | Yes | Limited |
| Interact with custom controls | Yes | No |
| Access web page DOM structure in browsers | Yes (AXWebArea) | Only via JS execution |
| Batch attribute reads (3-5x faster) | Yes (AXUIElementCopyMultipleAttributeValues) | No |

---

## 2. macOS System Services & APIs

### Wi-Fi Control

```bash
# Check current network
networksetup -getairportnetwork en0

# Toggle Wi-Fi
networksetup -setairportpower en0 on
networksetup -setairportpower en0 off

# Connect to network
networksetup -setairportnetwork en0 "SSID" "PASSWORD"

# List preferred networks
networksetup -listpreferredwirelessnetworks en0

# Manage preferred networks
networksetup -addpreferredwirelessnetworkatindex en0 "SSID" 0 WPA2
networksetup -removepreferredwirelessnetwork en0 "SSID"
```
**Permission:** None for read, admin for some writes.

### Bluetooth Control

```bash
# Requires: brew install blueutil
blueutil --power 0          # turn off
blueutil --power 1          # turn on
blueutil --discoverable 1   # make discoverable
blueutil --paired            # list paired devices
blueutil --connected         # list connected devices
blueutil --connect XX:XX:XX:XX:XX:XX  # connect device
blueutil --disconnect XX:XX:XX:XX:XX:XX

# System info
system_profiler SPBluetoothDataType
```

### Network (Full networksetup Command Set)

```bash
# IP configuration
networksetup -getinfo "Wi-Fi"
networksetup -setmanual "Wi-Fi" 192.168.1.100 255.255.255.0 192.168.1.1
networksetup -setdhcp "Wi-Fi"

# DNS
networksetup -getdnsservers "Wi-Fi"
networksetup -setdnsservers "Wi-Fi" 8.8.8.8 8.8.4.4

# Proxy
networksetup -getwebproxy "Wi-Fi"
networksetup -setwebproxy "Wi-Fi" proxy.example.com 8080
networksetup -setwebproxystate "Wi-Fi" on
networksetup -setsocksfirewallproxy "Wi-Fi" proxy.example.com 1080
networksetup -setproxybypassdomains "Wi-Fi" localhost 127.0.0.1

# VPN (requires existing VPN config)
networksetup -connectpppoeservice "My VPN"
networksetup -disconnectpppoeservice "My VPN"

# Location profiles
networksetup -getcurrentlocation
networksetup -listlocations
networksetup -createlocation "Office" populate
networksetup -switchtolocation "Office"

# Network service management
networksetup -listallnetworkservices
networksetup -setnetworkserviceenabled "Wi-Fi" on
networksetup -ordernetworkservices "Wi-Fi" "Ethernet"

# MAC address, MTU, VLAN
networksetup -getmacaddress en0
networksetup -setMTU en0 1500
```
**Permission:** Some operations require admin. Network changes are immediate.

### Sound / Volume Control

```bash
# Get volume
osascript -e 'output volume of (get volume settings)'    # 0-100
osascript -e 'get volume settings'                         # full info

# Set volume
osascript -e 'set volume output volume 50'                 # 0-100
osascript -e 'set volume output muted true'                # mute
osascript -e 'set volume output muted false'               # unmute
osascript -e 'set volume alert volume 0'                   # silence alert sounds
osascript -e 'set volume input volume 80'                  # input volume

# Audio devices
system_profiler SPAudioDataType

# Play sounds
afplay /System/Library/Sounds/Ping.aiff
afplay -v 0.5 sound.mp3                                    # at 50% volume
```
**Permission:** None.

### Display / Screen

```bash
# Display info
system_profiler SPDisplaysDataType

# Brightness (requires brew install brightness)
brightness 0.7                          # set to 70%
brightness -l                           # list displays

# Via IOKit (no brew needed, but complex)
# ioreg -c AppleBacklightDisplay | grep brightness

# Night Shift: No public CLI
# Requires CoreBrightness private framework or UI scripting

# Resolution: No simple CLI
# Can use displayplacer (brew install displayplacer)
# displayplacer list
# displayplacer "id:DISPLAY_ID res:1920x1080 hz:60 color_depth:8 scaling:on"
```

### Dark Mode

```bash
# Check current
osascript -e 'tell app "System Events" to get dark mode of appearance preferences'

# Toggle
osascript -e 'tell app "System Events" to set dark mode of appearance preferences to true'
osascript -e 'tell app "System Events" to set dark mode of appearance preferences to false'

# Via defaults (requires logout or app restart)
defaults write NSGlobalDomain AppleInterfaceStyle -string 'Dark'
defaults delete NSGlobalDomain AppleInterfaceStyle  # light mode
```

### Clipboard / Pasteboard

```bash
# Text
echo "hello" | pbcopy              # copy text
pbpaste                             # paste text
pbpaste -Prefer public.html        # paste as HTML
pbpaste -Prefer public.rtf         # paste as RTF

# Files (via AppleScript)
osascript -e 'set the clipboard to POSIX file "/path/to/file"'
osascript -e 'POSIX path of (the clipboard as alias)'

# Images (via AppleScript)
osascript -e 'set the clipboard to (read (POSIX file "/path/to/image.png") as TIFF picture)'

# Rich content types via JXA
osascript -l JavaScript -e '
ObjC.import("AppKit");
var pb = $.NSPasteboard.generalPasteboard;
// Read all types
var types = pb.types;
// Write HTML
pb.clearContents;
pb.setStringForType($("Hello"), $("public.html"));
// Write multiple types simultaneously
pb.declareTypesOwner($([$.NSPasteboardTypeString, $.NSPasteboardTypeHTML]), null);
pb.setStringForType($("plain text"), $.NSPasteboardTypeString);
pb.setStringForType($("<b>rich</b>"), $.NSPasteboardTypeHTML);
'

# Clipboard info
osascript -e 'tell app "System Events" to clipboard info'
```
**Permission:** None. Universal clipboard syncs automatically between Apple devices.

### Keychain

```bash
# Find password
security find-generic-password -a 'account' -s 'service' -w
security find-internet-password -s 'server.com' -w

# Add password
security add-generic-password -a 'account' -s 'service' -w 'password'
security add-internet-password -a 'user' -s 'server.com' -w 'password'

# Delete
security delete-generic-password -a 'account' -s 'service'

# List keychains
security list-keychains
security dump-keychain     # dump all items (without passwords)

# Certificates
security find-certificate -a -p  # dump all certificates as PEM
security find-identity -p codesigning  # find signing identities

# Unlock keychain
security unlock-keychain -p 'password' ~/Library/Keychains/login.keychain-db
```
**Permission:** Accessing passwords triggers a GUI prompt unless the calling app is in the keychain item's ACL. The user must click "Allow" or "Always Allow".

**Edge case:** `security add-generic-password` adds silently. But `find-generic-password -w` may prompt. For automation, you can pre-authorize by adding the terminal to the ACL: `security set-generic-password-partition-list -s "service" -a "account" -S "apple-tool:,apple:,teamid:TEAM_ID"`.

### Power Management

```bash
# Sleep
pmset sleepnow
osascript -e 'tell app "System Events" to sleep'

# Prevent sleep
caffeinate -t 3600        # prevent for 1 hour
caffeinate -i             # prevent idle sleep (until killed)
caffeinate -d             # prevent display sleep
caffeinate -s             # prevent system sleep on AC power

# Shutdown/Restart (requires admin or triggers GUI dialog)
osascript -e 'tell app "System Events" to shut down'
osascript -e 'tell app "System Events" to restart'
osascript -e 'tell app "System Events" to log out'
sudo shutdown -h now      # immediate shutdown
sudo shutdown -r now      # immediate restart

# Power info
pmset -g                  # current settings
pmset -g batt             # battery status
pmset -g assertions       # what's preventing sleep

# Schedule
sudo pmset schedule wake "04/03/2026 08:00:00"
sudo pmset schedule sleep "04/03/2026 23:00:00"
```

### Notifications

```bash
# Create notification
osascript -e 'display notification "Body" with title "Title" subtitle "Subtitle" sound name "Ping"'

# Rich notifications (requires terminal-notifier: brew install terminal-notifier)
terminal-notifier -message "Hello" -title "Title" -open "https://google.com"
terminal-notifier -message "Hello" -execute "echo clicked"
terminal-notifier -message "Hello" -appIcon /path/to/icon.png
terminal-notifier -message "Hello" -sound "Basso"
terminal-notifier -message "Hello" -group "my-group"  # replaceable notification

# Read notifications: No public API
# Workaround: AX tree of Notification Center when banners are visible
# Database at ~/Library/GroupContainers/group.com.apple.usernoted/db2/db (SIP-protected)
```

### Preferences Database (`defaults`)

The `defaults` command can read/write ANY application's preferences:

```bash
# Read
defaults read com.apple.finder             # all Finder prefs
defaults read NSGlobalDomain               # all global prefs
defaults read -g                           # alias for NSGlobalDomain

# Write
defaults write com.apple.finder AppleShowAllFiles -bool true
defaults write com.apple.dock autohide -bool true && killall Dock
defaults write NSGlobalDomain KeyRepeat -int 1

# Delete
defaults delete com.apple.dock autohide

# Types: -bool, -int, -float, -string, -array, -dict, -data
```

**Key `defaults` for system control:**

| Domain | Key | Values | Effect |
|--------|-----|--------|--------|
| NSGlobalDomain | AppleInterfaceStyle | "Dark" / delete | Dark/Light mode |
| NSGlobalDomain | KeyRepeat | 1-15 | Key repeat speed |
| NSGlobalDomain | InitialKeyRepeat | 10-120 | Key repeat delay |
| NSGlobalDomain | AppleShowAllExtensions | true/false | Show file extensions |
| NSGlobalDomain | NSAutomaticSpellingCorrectionEnabled | true/false | Auto-correct |
| com.apple.finder | AppleShowAllFiles | true/false | Show hidden files |
| com.apple.finder | ShowPathbar | true/false | Path bar |
| com.apple.finder | _FXSortFoldersFirst | true/false | Folders on top |
| com.apple.dock | autohide | true/false | Auto-hide dock |
| com.apple.dock | tilesize | 16-128 | Dock icon size |
| com.apple.dock | show-recents | true/false | Recent apps in dock |
| com.apple.dock | mru-spaces | true/false | Auto-rearrange spaces |
| com.apple.screencapture | location | path string | Screenshot save location |
| com.apple.screencapture | type | "png"/"jpg"/"pdf" | Screenshot format |
| com.apple.screencapture | disable-shadow | true/false | Window shadow in screenshots |
| com.apple.Safari | IncludeDevelopMenu | true/false | Safari developer tools |

**Note:** Most `defaults` changes require killing the app (`killall Finder`) or logging out to take effect.

### Firewall

```bash
# Check status
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --getglobalstate

# Enable/disable
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setglobalstate on
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setglobalstate off

# App rules
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add /path/to/app
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --blockapp /path/to/app
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --unblockapp /path/to/app
```

### User Management

```bash
dscl . -list /Users                         # list users
dscl . -read /Users/username                # user details
whoami                                       # current user
id                                           # uid, gid, groups
sudo sysadminctl -addUser newuser -fullName "Full Name" -password "pass"
sudo sysadminctl -deleteUser olduser
dscl . -passwd /Users/username newpassword
```

---

## 3. macOS Shortcuts / Automator

### Shortcuts CLI

```bash
# List all shortcuts
shortcuts list

# Run a shortcut
shortcuts run "Shortcut Name"

# Run with file input
shortcuts run "Shortcut Name" --input-path /path/to/file.txt

# Run with stdin input
echo "text" | shortcuts run "Shortcut Name"

# View shortcut in editor
shortcuts view "Shortcut Name"

# Sign for sharing
shortcuts sign -i unsigned.shortcut -o signed.shortcut -m anyone
```

**Cannot create shortcuts programmatically.** But can import `.shortcut` files via `open file.shortcut`.

**Unique Shortcuts integrations (hard to access otherwise):**
- HomeKit device control
- Focus mode toggle (no other CLI can do this)
- Wallet / Apple Pay passes
- Health data access
- Location services
- Siri integration
- Third-party App Intents
- AirDrop
- Markup / annotations

**Reliability:** Good. The `shortcuts run` command is synchronous and returns exit code 0 on success. Stderr contains errors. Stdout receives shortcut output.

### Automator

```bash
# Run automator workflow
automator /path/to/workflow.workflow

# Run via open
open /path/to/workflow.workflow
```

**Automator actions provide pre-built capabilities for:**
- PDF manipulation (merge, split, watermark)
- Image manipulation (crop, resize, filter)
- File renaming patterns
- Email operations
- Calendar operations
- Disk image creation

**Status:** Automator is effectively deprecated in favor of Shortcuts. Still works but not receiving new features.

---

## 4. Launch Services & App Management

### The `open` Command (Extremely Powerful)

```bash
# Launch apps
open /Applications/Safari.app
open -a "TextEdit"                     # by name
open -b com.apple.Safari               # by bundle ID
open -g /Applications/Safari.app       # launch in background
open -n /Applications/Safari.app       # new instance (even if already running)
open -j                                # launch hidden

# Open files
open file.pdf                          # default app
open -a "TextEdit" file.txt            # specific app
open -e file.txt                       # TextEdit specifically
open -t file.txt                       # default text editor
open -R file.txt                       # reveal in Finder
open --stdin /Applications/TextEdit.app < file.txt  # pipe content

# Open URLs
open "https://google.com"
open "mailto:user@example.com?subject=Hello&body=World"
open "facetime://user@example.com"
open "tel://+1234567890"
open "sms://+1234567890"
open "imessage://user@example.com"
open "shortcuts://run-shortcut?name=MyShortcut"
open "music://album/12345"
open "maps://?q=coffee+near+me"
open "x-apple.systempreferences:com.apple.preference.security"

# Open System Settings panes
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
open "x-apple.systempreferences:com.apple.preference.network"
open "x-apple.systempreferences:com.apple.preference.displays"
open "x-apple.systempreferences:com.apple.preference.sound"
```

### App Metadata

```bash
# Bundle ID
mdls -name kMDItemCFBundleIdentifier /Applications/Safari.app
defaults read /Applications/App.app/Contents/Info.plist CFBundleIdentifier

# Running app info
lsappinfo list                          # all running apps
lsappinfo info -app "com.apple.Safari"  # specific app

# App version, copyright, etc.
mdls /Applications/Safari.app
```

### App Installation

```bash
# DMG
hdiutil attach app.dmg
cp -R "/Volumes/AppName/App.app" /Applications/
hdiutil detach "/Volumes/AppName"

# PKG
sudo installer -pkg package.pkg -target /

# Mac App Store (requires: brew install mas)
mas search "app name"
mas install 497799835
mas upgrade

# Homebrew
brew install --cask app-name
brew uninstall --cask app-name

# Uninstall
rm -rf /Applications/App.app
# Some apps also store data in:
# ~/Library/Application Support/AppName
# ~/Library/Preferences/com.company.app.plist
# ~/Library/Caches/com.company.app
```

### Login Items

```bash
# List
osascript -e 'tell app "System Events" to get name of every login item'

# Add
osascript -e 'tell app "System Events" to make login item at end with properties {path:"/Applications/App.app", hidden:false}'

# Remove
osascript -e 'tell app "System Events" to delete login item "AppName"'
```

---

## 5. File System Deep Control

### Finder Tags

```bash
# Via xattr (complex binary plist encoding)
xattr -p com.apple.metadata:_kMDItemUserTags file.txt

# Via tag CLI (brew install tag) — recommended
tag -a Red file.txt          # add tag
tag -a "Custom Tag" file.txt # custom tag
tag -r Red file.txt          # remove tag
tag -l file.txt              # list tags
tag -f Red                   # find all files with tag
tag -f Red ~/Documents       # find in directory

# Via AppleScript (Finder)
osascript -e 'tell app "Finder" to get label index of (POSIX file "/path" as alias)'
osascript -e 'tell app "Finder" to set label index of (POSIX file "/path" as alias) to 6'
# Label indices: 0=none, 1=orange, 2=red, 3=yellow, 4=blue, 5=purple, 6=green, 7=gray
```

### Finder Comments

```bash
# Read
osascript -e 'tell app "Finder" to get comment of (POSIX file "/path" as alias)'

# Write
osascript -e 'tell app "Finder" to set comment of (POSIX file "/path" as alias) to "My comment"'
```

### Spotlight Search (mdfind/mdls)

```bash
# Find by content/name
mdfind "search query"                           # full-text search
mdfind -name "report"                           # filename search
mdfind -onlyin ~/Documents "budget"             # search in directory

# Find by metadata
mdfind 'kMDItemKind == "PDF Document"'
mdfind 'kMDItemContentType == "public.image"'
mdfind 'kMDItemPixelWidth > 1920'
mdfind 'kMDItemContentCreationDate > $time.today(-7)'   # last 7 days
mdfind 'kMDItemContentModificationDate > $time.today(-1)'  # last 24h
mdfind 'kMDItemFSSize > 100000000'              # files > 100MB
mdfind 'kMDItemWhereFroms == "*.github.com*"'   # downloaded from GitHub
mdfind 'kMDItemContentType == "com.apple.mail.emlx"'  # emails

# Get file metadata
mdls file.txt                                    # all metadata
mdls -name kMDItemWhereFroms ~/Downloads/file.zip  # download source URL
mdls -name kMDItemContentType file.txt
mdls -name kMDItemPixelHeight -name kMDItemPixelWidth image.png

# Force re-index
mdimport file.txt
sudo mdutil -E /                                 # re-index entire drive
```

### Extended Attributes (xattr)

```bash
# List all xattrs
xattr -l file.txt

# Read specific
xattr -p com.apple.quarantine file.txt           # quarantine flag (from download)
xattr -p com.apple.metadata:_kMDItemUserTags file.txt

# Write
xattr -w custom.key "value" file.txt

# Remove
xattr -d com.apple.quarantine file.txt           # remove quarantine (make executable)
xattr -c file.txt                                 # remove ALL xattrs

# Recursive
xattr -rc /path/to/directory                      # remove all xattrs recursively
```

### Trash

```bash
# Move to trash (Finder)
osascript -e 'tell app "Finder" to delete POSIX file "/path/to/file"'

# Empty trash
osascript -e 'tell app "Finder" to empty the trash'

# Trash location
ls ~/.Trash/

# Move to trash via command line (macOS 12+)
# No built-in CLI command. Use: brew install trash
# trash file.txt
```

### Disk Management

```bash
diskutil list                          # list all disks/partitions
diskutil info disk0                    # detailed disk info
diskutil mount disk2s1                 # mount partition
diskutil unmount disk2s1               # unmount
diskutil eject disk2                   # eject external drive
diskutil apfs list                     # APFS container info
diskutil verifyVolume /                # verify disk
diskutil repairVolume /                # repair disk (requires recovery mode)

# DMG operations
hdiutil create -size 1g -fs APFS -volname "MyDisk" disk.dmg
hdiutil attach disk.dmg                # mount DMG
hdiutil detach /Volumes/MyDisk         # unmount DMG
hdiutil convert disk.dmg -format UDRW -o writable.dmg  # make writable

# Disk usage
du -sh /path                           # directory size
df -h                                  # free space
```

### Document Conversion (textutil)

```bash
textutil -convert html document.docx   # DOCX to HTML
textutil -convert txt document.docx    # DOCX to plain text
textutil -convert rtf document.html    # HTML to RTF
textutil -convert docx document.rtf    # RTF to DOCX
textutil -convert pdf document.rtf     # RTF to PDF
textutil -cat txt *.txt -output combined.txt  # concatenate files
# Supports: txt, rtf, rtfd, html, doc, docx, odt, wordml
```

### Property List Operations (plutil)

```bash
plutil -convert json Info.plist -o Info.json  # plist to JSON
plutil -convert xml1 prefs.plist              # binary to XML plist
plutil -replace key -string "value" file.plist
plutil -insert key -string "value" file.plist
plutil -remove key file.plist
plutil -lint file.plist                        # validate
```

### Advanced Copy (ditto)

```bash
ditto source dest                              # copy preserving all metadata
ditto -c -k --sequesterRsrc source archive.zip  # create zip
ditto -x -k archive.zip dest                   # extract zip
ditto --hfsCompression source dest             # copy with HFS+ compression
```

---

## 6. Input Methods Beyond Basic Mouse/Keyboard

### CGEvent Mouse Events (via JXA)

```javascript
// All mouse event types available:
// kCGEventLeftMouseDown, kCGEventLeftMouseUp, kCGEventLeftMouseDragged
// kCGEventRightMouseDown, kCGEventRightMouseUp, kCGEventRightMouseDragged
// kCGEventOtherMouseDown, kCGEventOtherMouseUp, kCGEventOtherMouseDragged
// kCGEventMouseMoved, kCGEventScrollWheel

ObjC.import("CoreGraphics");

// Left click
var down = $.CGEventCreateMouseEvent(null, $.kCGEventLeftMouseDown, $.CGPointMake(x, y), $.kCGMouseButtonLeft);
var up = $.CGEventCreateMouseEvent(null, $.kCGEventLeftMouseUp, $.CGPointMake(x, y), $.kCGMouseButtonLeft);
$.CGEventPost($.kCGHIDEventTap, down);
$.CGEventPost($.kCGHIDEventTap, up);

// Double-click
$.CGEventSetIntegerValueField(down, $.kCGMouseEventClickState, 2);
$.CGEventSetIntegerValueField(up, $.kCGMouseEventClickState, 2);

// Right-click (context menu)
var rDown = $.CGEventCreateMouseEvent(null, $.kCGEventRightMouseDown, $.CGPointMake(x, y), $.kCGMouseButtonRight);
var rUp = $.CGEventCreateMouseEvent(null, $.kCGEventRightMouseUp, $.CGPointMake(x, y), $.kCGMouseButtonRight);

// Ctrl+Click (alternative right-click)
$.CGEventSetFlags(down, $.kCGEventFlagMaskControl);
```

### Drag and Drop

```javascript
ObjC.import("CoreGraphics");

// 1. Mouse down at source
var mouseDown = $.CGEventCreateMouseEvent(null, $.kCGEventLeftMouseDown,
    $.CGPointMake(sourceX, sourceY), $.kCGMouseButtonLeft);
$.CGEventPost($.kCGHIDEventTap, mouseDown);

// 2. Small delay, then drag (send multiple intermediate points for reliability)
for (var i = 0; i <= 10; i++) {
    var t = i / 10;
    var x = sourceX + (destX - sourceX) * t;
    var y = sourceY + (destY - sourceY) * t;
    var drag = $.CGEventCreateMouseEvent(null, $.kCGEventLeftMouseDragged,
        $.CGPointMake(x, y), $.kCGMouseButtonLeft);
    $.CGEventPost($.kCGHIDEventTap, drag);
    // delay(0.01) between points
}

// 3. Mouse up at destination
var mouseUp = $.CGEventCreateMouseEvent(null, $.kCGEventLeftMouseUp,
    $.CGPointMake(destX, destY), $.kCGMouseButtonLeft);
$.CGEventPost($.kCGHIDEventTap, mouseUp);
```
**Reliability:** Works well for most apps. Some apps require the drag to be slow enough (10-50ms between points). Spring-loaded folders in Finder require hovering for ~0.5s.

### Scroll

```javascript
// Scroll wheel (pixel-based)
var scroll = $.CGEventCreateScrollWheelEvent(null, $.kCGScrollEventUnitPixel, 1, -50);
// Positive = scroll up, negative = scroll down
$.CGEventPost($.kCGHIDEventTap, scroll);

// Horizontal scroll
var hscroll = $.CGEventCreateScrollWheelEvent(null, $.kCGScrollEventUnitPixel, 2, 0, -50);
```

### Multi-touch Gestures

**NOT possible via public CGEvent API.**

Options:
1. **Private MultitouchSupport.framework** -- undocumented, breaks between OS versions
2. **Keyboard shortcuts** as workaround:
   - Pinch zoom: Cmd+Plus/Minus
   - Mission Control: Ctrl+Up
   - Switch spaces: Ctrl+Left/Right
   - App Expose: Ctrl+Down
   - Show Desktop: (no standard shortcut, assign in System Settings)
3. **yabai** for window management gestures

### Force Touch

```javascript
// Pressure is a double value 0.0-1.0
$.CGEventSetDoubleValueField(event, $.kCGMouseEventPressure, 0.8);
// But true "Force Touch" (deep press) requires private SkyLight APIs
```
**Workaround:** Force Touch in most contexts triggers "Quick Look" or "Look Up" which can be invoked via:
- AX action: `AXShowDefaultUI`
- Keyboard: Spacebar in Finder = Quick Look

### File Picker / Save Dialog Automation

```applescript
tell application "System Events"
    tell process "AppName"
        -- Wait for Open/Save sheet
        repeat until exists sheet 1 of window 1
            delay 0.1
        end repeat

        -- Navigate using Go To Folder (Cmd+Shift+G)
        keystroke "g" using {command down, shift down}
        delay 0.5
        keystroke "/exact/path/to/file"
        keystroke return
        delay 0.5
        keystroke return  -- click Open/Save
    end tell
end tell
```
**Alternative:** Use AX tree to find the path bar / text field in the dialog, then set its value directly.

### Text Selection

```bash
# Via AX API (most reliable)
# Set AXSelectedTextRange attribute on a text area to select text range

# Via keyboard (CGEvent)
# Cmd+A = select all
# Shift+Arrow = extend selection
# Cmd+Shift+End = select to end
# Triple-click = select paragraph

# Via AppleScript System Events
osascript -e 'tell app "System Events" to tell process "TextEdit"
    keystroke "a" using command down  -- select all
end tell'
```

---

## 7. Process & Window Management

### Window Listing

```javascript
// Via JXA + CoreGraphics
ObjC.import("CoreGraphics");
var windowList = $.CGWindowListCopyWindowInfo(
    $.kCGWindowListOptionOnScreenOnly, $.kCGNullWindowID);
// Returns: owner name, window name, bounds, layer, PID, windowID
```

```bash
# Via AppleScript
osascript -e 'tell app "System Events" to get {name, position, size} of every window of every process whose visible is true'
```

### Window Positioning & Sizing

```bash
# Via AppleScript (specific app)
osascript -e 'tell app "Finder" to set bounds of window 1 to {0, 0, 800, 600}'
osascript -e 'tell app "Finder" to set position of window 1 to {100, 100}'

# Via System Events (any app)
osascript -e 'tell app "System Events" to tell process "Finder"
    set position of window 1 to {0, 0}
    set size of window 1 to {800, 600}
end tell'

# Via AX API (from Swift/native code)
# AXUIElementSetAttributeValue(element, kAXPositionAttribute, pointValue)
# AXUIElementSetAttributeValue(element, kAXSizeAttribute, sizeValue)
```

### Window Actions

```bash
# Minimize
osascript -e 'tell app "System Events" to tell process "Finder"
    set value of attribute "AXMinimized" of window 1 to true
end tell'

# Fullscreen toggle
osascript -e 'tell app "System Events" to tell process "Finder"
    set value of attribute "AXFullScreen" of window 1 to true
end tell'

# Bring to front
osascript -e 'tell app "Finder" to activate'

# Close window
osascript -e 'tell app "System Events" to tell process "Finder"
    click button 1 of window 1  -- close button
end tell'

# Zoom (maximize)
osascript -e 'tell app "System Events" to tell process "Finder"
    click button 3 of window 1  -- zoom button
end tell'
```

### Spaces / Mission Control

```bash
# Switch spaces via keyboard
osascript -e 'tell app "System Events" to key code 123 using control down'  # left space
osascript -e 'tell app "System Events" to key code 124 using control down'  # right space

# Go to specific space (requires keyboard shortcuts configured)
osascript -e 'tell app "System Events" to key code 18 using control down'   # Ctrl+1 = Space 1
osascript -e 'tell app "System Events" to key code 19 using control down'   # Ctrl+2 = Space 2

# Mission Control
osascript -e 'tell app "System Events" to key code 126 using control down'  # Ctrl+Up
# Or
open -b 'com.apple.exposelauncher'

# Read spaces configuration
defaults read com.apple.spaces

# Private APIs (used by yabai, require SIP modification)
# CGSCopySpaces(), CGSMoveWindowToSpace(), CGSGetActiveSpace()
# SLSCopySpacesForWindows(), SLSMoveWindowsToManagedSpace()
```

**Limitations:** Public APIs cannot create/delete spaces or move windows between spaces. This requires either:
1. yabai with SIP partially disabled
2. Private SkyLight.framework APIs
3. Keyboard shortcut simulation (unreliable)

### Tab Management in Apps

```bash
# Safari tabs
osascript -e 'tell app "Safari" to get name of every tab of window 1'
osascript -e 'tell app "Safari" to set current tab of window 1 to tab 3 of window 1'
osascript -e 'tell app "Safari" to make new tab at end of tabs of window 1 with properties {URL:"https://example.com"}'
osascript -e 'tell app "Safari" to close tab 2 of window 1'

# Chrome tabs
osascript -e 'tell app "Google Chrome" to get title of every tab of window 1'
osascript -e 'tell app "Google Chrome" to get URL of active tab of window 1'
osascript -e 'tell app "Google Chrome" to set active tab index of window 1 to 3'
osascript -e 'tell app "Google Chrome" to execute active tab of window 1 javascript "document.title"'
osascript -e 'tell app "Google Chrome" to make new tab at end of tabs of window 1 with properties {URL:"https://example.com"}'

# Other apps: Cmd+T (new tab), Cmd+W (close tab), Ctrl+Tab (next tab)
```

### App Management

```bash
# List running apps
osascript -e 'tell app "System Events" to get name of every process whose background only is false'

# Quit app
osascript -e 'tell app "Safari" to quit'

# Force quit
osascript -e 'tell app "Safari" to quit saving no'
killall Safari
kill -9 $(pgrep Safari)

# Hide/Show
osascript -e 'tell app "System Events" to set visible of process "Finder" to false'
osascript -e 'tell app "Finder" to activate'  # show/bring to front

# Check if running
osascript -e 'tell app "System Events" to (name of every process) contains "Safari"'
pgrep -x Safari
```

---

## 8. Inter-Process Communication

### URL Schemes

```bash
# System apps
open "https://example.com"                                    # default browser
open "mailto:user@example.com?subject=Hi&body=Hello"         # Mail
open "tel://+1234567890"                                      # FaceTime
open "facetime://user@example.com"                            # FaceTime video
open "facetime-audio://user@example.com"                      # FaceTime audio
open "sms://+1234567890"                                      # Messages
open "imessage://user@example.com"                            # iMessage
open "maps://?q=coffee+near+me"                               # Maps
open "music://album/12345"                                    # Music
open "shortcuts://run-shortcut?name=MyShortcut&input=text"    # Shortcuts
open "notes://showNote?identifier=xxx"                        # Notes
open "x-apple-reminder://"                                    # Reminders
open "calshow:UNIX_TIMESTAMP"                                 # Calendar (show date)

# System Settings (macOS 13+)
open "x-apple.systempreferences:com.apple.preference.security"
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
open "x-apple.systempreferences:com.apple.preference.network"
open "x-apple.systempreferences:com.apple.preference.displays"
open "x-apple.systempreferences:com.apple.preference.sound"
open "x-apple.systempreferences:com.apple.preference.keyboard"
open "x-apple.systempreferences:com.apple.preference.general"

# Third-party x-callback-url pattern
open "things:///add?title=New%20Task&notes=Details&when=today"
open "bear://x-callback-url/create?title=Note&text=Content"
open "drafts://x-callback-url/create?text=Content"
open "obsidian://new?name=Note&content=Content"
```

### Distributed Notifications

```javascript
// Post (via JXA)
ObjC.import("Foundation");
$.NSDistributedNotificationCenter.defaultCenter
    .postNotificationNameObjectUserInfoDeliverImmediately(
        "com.myapp.event", null, null, true);

// Observe: requires long-running process (Swift/ObjC)
// Cannot observe from CLI one-shot command
```

### Services Menu

Apps register in `Info.plist` under `NSServices`. The system Services menu provides cross-app data sharing:
- Select text > Services > Make Sticky Note
- Select text > Services > Look Up in Dictionary
- Select file > Services > Send via AirDrop

Can invoke via AppleScript UI scripting of the Services menu.

### Apple Events (direct app-to-app)

```bash
# Send Apple Event to app
osascript -e 'tell app "Safari" to open location "https://example.com"'

# The underlying mechanism:
# NSAppleEventDescriptor / AESendMessage
# Every osascript command uses Apple Events under the hood
```

---

## 9. Terminal / Shell Integration

```bash
# Run any command
/bin/bash -c "command"
/bin/zsh -c "command"

# Run with admin privileges (triggers GUI password dialog)
osascript -e 'do shell script "command" with administrator privileges'

# Environment
printenv                    # all env vars
export KEY=VALUE            # set for session
launchctl setenv KEY VALUE  # set for GUI apps

# Package managers
brew install/uninstall/upgrade/search
npm install -g / yarn global add
pip install / pip3 install
cargo install

# Docker
docker run/build/compose/exec

# SSH
ssh user@host "command"
ssh -L local:host:remote user@host  # port forwarding

# Process management
nohup command &             # run in background, survive logout
screen / tmux               # terminal multiplexer
launchctl load/unload       # manage launchd services

# Cron / scheduled tasks
crontab -e                  # edit crontab
launchctl load ~/Library/LaunchAgents/com.me.task.plist  # launchd (preferred)
```

---

## 10. Media & Creative Tools

### Screenshots

```bash
screencapture -x screenshot.png          # full screen, silent
screencapture -c                         # to clipboard
screencapture -i screenshot.png          # interactive selection
screencapture -w screenshot.png          # window capture (click to select)
screencapture -R 0,0,800,600 shot.png   # specific region
screencapture -l WINDOW_ID shot.png     # specific window by CGWindowID
screencapture -D 2 shot.png             # specific display (multi-monitor)
screencapture -T 5 shot.png             # 5 second delay
screencapture -t jpg shot.jpg           # format: png, jpg, pdf, tiff, gif
screencapture -x -t jpg -R 0,0,800,600 -o region.jpg  # combined options (-o = no shadow)
```
**Permission:** Screen Recording.
**Via ScreenCaptureKit (Swift):** Hardware-accelerated, 5-15ms per capture, can capture individual windows without bringing to front.

### Screen Recording

```bash
# Native (macOS 14+)
screencapture -v -D 1 recording.mov     # record display 1

# Via ScreenCaptureKit (Swift): can record specific window/app/region
# SCStream, SCStreamConfiguration, SCContentFilter
# Output: H.264/HEVC, hardware-accelerated
```
**Permission:** Screen Recording.

### Camera / Microphone

```bash
# List devices
system_profiler SPCameraDataType

# Capture photo (brew install imagesnap)
imagesnap photo.jpg
imagesnap -t 2 photo.jpg               # 2 second delay
imagesnap -d "FaceTime HD Camera"       # specific camera

# Video recording (ffmpeg)
# ffmpeg -f avfoundation -i "0:0" -t 10 output.mp4  # 10 second recording

# Audio recording
# sox (brew install sox)
# rec output.wav trim 0 10             # record 10 seconds
```
**Permission:** Camera, Microphone.

### Image Manipulation (sips)

```bash
sips -z 600 800 image.png               # resize to 600x800
sips -Z 1024 image.png                  # resize max dimension preserving aspect
sips -s format jpeg image.png --out image.jpg  # convert format
sips -r 90 image.png                    # rotate 90 degrees
sips -f horizontal image.png            # flip horizontal
sips -f vertical image.png              # flip vertical
sips -g pixelHeight -g pixelWidth image.png  # get dimensions
sips -g all image.png                   # all properties
sips -i image.png                       # set file icon to image content
sips -s dpiHeight 300 -s dpiWidth 300 image.png  # set DPI
sips --resampleWidth 1024 image.png     # resample to width

# Batch operations
sips -Z 800 *.png                       # resize all PNGs
sips -s format jpeg *.png --out /output/  # convert all to JPEG

# Supported formats: JPEG, TIFF, PNG, GIF, BMP, PDF, PSD, ICO, HEIC, JPEG-2000
```

### PDF Manipulation

```bash
# Built-in Python + Quartz
python3 << 'EOF'
import Quartz
# Open PDF
pdf = Quartz.PDFDocument.alloc().initWithURL_(
    Quartz.NSURL.fileURLWithPath_("/path/to/file.pdf"))
# Page count
print(pdf.pageCount())
# Extract text from page
page = pdf.pageAtIndex_(0)
print(page.string())
EOF

# PDF metadata
mdls document.pdf

# Convert to PDF
textutil -convert pdf document.rtf
cupsfilter input.txt > output.pdf 2>/dev/null

# PDF tools (brew install poppler)
pdftotext document.pdf output.txt       # PDF to text
pdfimages document.pdf images/          # extract images
pdfinfo document.pdf                    # metadata
pdfseparate document.pdf page_%d.pdf    # split pages
pdfunite page_1.pdf page_2.pdf merged.pdf  # merge
```

### Audio

```bash
# Text to speech
say "Hello World"
say -v Samantha "Hello World"           # specific voice
say -o output.aiff "Hello World"        # save to file
say -r 200 "Fast speech"               # speech rate

# List voices
say -v '?'

# Play audio
afplay sound.mp3
afplay -v 0.5 sound.mp3                # at 50% volume
afplay -t 5 sound.mp3                  # play 5 seconds

# Audio info
afinfo sound.mp3

# Convert
afconvert input.wav output.m4a -d aac -f m4af
afconvert input.wav output.mp3 -d mp3
```

### Quick Look

```bash
qlmanage -p file.pdf                    # preview
qlmanage -t file.pdf -s 600 -o /tmp/   # generate thumbnail
qlmanage -x file.pdf                    # metadata as XML
```

---

## 11. Private / Undocumented APIs

These are used by power-user tools (yabai, BetterTouchTool, Karabiner Elements) but may break between OS versions.

### SkyLight.framework (Window Server)

```
SLSCopySpacesForWindows()           -- get spaces containing windows
SLSMoveWindowsToManagedSpace()      -- move window to space
SLSSetWindowAlpha()                 -- set window transparency
SLSSetWindowLevel()                 -- set window z-order
SLSDisableUpdate() / SLSReenableUpdate()  -- batch operations
CGSCopyManagedDisplaySpaces()       -- list all spaces
CGSGetActiveSpace()                 -- current space ID
CGSHideSpaces() / CGSShowSpaces()   -- hide/show spaces
```
**Requires:** SIP partially disabled for yabai (`csrutil enable --without debug`). Window transparency and level changes work without SIP.

### CoreBrightness.framework (Night Shift)

```
CBBlueLightClient.setEnabled(_:)     -- toggle Night Shift
CBBlueLightClient.setStrength(_:)    -- set intensity (0.0-1.0)
CBBlueLightClient.setSchedule(_:)    -- set schedule
```
**Works without SIP.** Used by NightShiftControl and similar utilities.

### IOKit (Hardware Access)

```
IOHIDPostEvent()                     -- synthesize HID events
IODisplaySetFloatParameter()         -- set display brightness
IOPMAssertionCreateWithProperties()  -- prevent sleep
IOServiceGetMatchingServices()       -- enumerate hardware
IORegistryEntryCreateCFProperties()  -- read hardware properties
```

### MultitouchSupport.framework

```
MTDeviceCreateDefault()              -- access trackpad
MTRegisterContactFrameCallback()     -- read touch events
MTActuateTouch()                     -- haptic feedback
```
**Used by:** BetterTouchTool, Karabiner Elements.

### Accessibility Private Extensions

```
_AXUIElementGetWindow()              -- get CGWindowID from AX element
_kAXFullScreenAttribute              -- fullscreen state
_kAXAnimatedAttribute                -- animation state
```

---

## 12. Complete Permissions Map

| Permission | Required For | Check Method | Grant Location |
|-----------|-------------|--------------|----------------|
| **Accessibility** | AX tree read/write, CGEvent input synthesis, UI scripting | `AXIsProcessTrusted()` | System Settings > Privacy & Security > Accessibility |
| **Screen Recording** | Screenshots via ScreenCaptureKit, screencapture | `CGPreflightScreenCaptureAccess()` | System Settings > Privacy & Security > Screen & System Audio Recording |
| **Input Monitoring** | CGEvent tap creation (intercept/record input) | `CGEvent.tapCreate()` returns nil if denied | System Settings > Privacy & Security > Input Monitoring |
| **Automation** | AppleScript control of specific apps | First use triggers dialog | System Settings > Privacy & Security > Automation |
| **Full Disk Access** | Read Mail database, Safari history, Messages, TCC.db | Try reading protected path | System Settings > Privacy & Security > Full Disk Access |
| **Camera** | Photo/video capture | `AVCaptureDevice.authorizationStatus()` | System Settings > Privacy & Security > Camera |
| **Microphone** | Audio recording | `AVCaptureDevice.authorizationStatus()` | System Settings > Privacy & Security > Microphone |
| **Location** | CoreLocation | `CLLocationManager.authorizationStatus()` | System Settings > Privacy & Security > Location Services |
| **Contacts** | CNContactStore | `CNContactStore.authorizationStatus()` | System Settings > Privacy & Security > Contacts |
| **Calendar** | EventKit | `EKEventStore.authorizationStatus()` | System Settings > Privacy & Security > Calendars |
| **Reminders** | EventKit | `EKEventStore.authorizationStatus()` | System Settings > Privacy & Security > Reminders |
| **Photos** | PhotoKit | `PHPhotoLibrary.authorizationStatus()` | System Settings > Privacy & Security > Photos |
| **Speech Recognition** | SFSpeechRecognizer | `SFSpeechRecognizer.authorizationStatus()` | System Settings > Privacy & Security > Speech Recognition |
| **Admin (sudo)** | System-level changes (shutdown, user mgmt, firewall) | `id -G \| grep -w 80` | User must be in admin group |

**Critical note:** Automation permission is **per-app-pair**. Terminal.app controlling Safari and Terminal.app controlling Finder are separate permission entries. Each triggers its own dialog on first use.

**Permissions are granted to the terminal app** (Terminal.app, iTerm, Ghostty), not to the CLI binary itself. If the user runs `cu` in iTerm, iTerm needs the permissions.

---

## 13. Gap Analysis

### What No Existing Tool Does Today

| Capability | agent-desktop | Ghost OS | axcli | cliclick | macOS Automator MCP |
|-----------|:---:|:---:|:---:|:---:|:---:|
| AX Tree perception | Yes | Yes | Yes | No | No |
| CGEvent input | Yes (fallback) | Yes (fallback) | Yes | Yes | No |
| AppleScript/JXA execution | No | No | No | No | Yes (recipes) |
| System settings (network, display, sound) | No | No | No | No | Partial |
| Shortcuts integration | No | No | No | No | No |
| File system deep (tags, Spotlight, xattr) | No | No | No | No | Partial |
| Multi-format clipboard | Partial | No | No | No | Partial |
| Screenshot + OCR | No | Yes (ShowUI) | Yes (Vision) | No | No |
| Browser JS execution | No | Yes (CDP) | No | No | Yes (recipes) |
| Drag & drop | Yes | No | No | Yes | No |
| Menu bar interaction | Via AX | Via AX | Via AX | No | Yes |
| **Unified CLI for all** | **No** | **No** | **No** | **No** | **No** |

**computer-pilot's unique value: be the first tool that unifies ALL of these into a single CLI.**

---

## 14. AppleScript vs AX Tree: When to Use Which

### Decision Matrix

```
Is the app scriptable AND you need semantic operations?
  (create event, send email, execute JS, manage playlist)
  → Use AppleScript/JXA

Is the app non-scriptable OR you need to interact with specific UI elements?
  (click button at position, read text field value, navigate custom UI)
  → Use AX Tree

Do you need to control system settings?
  (volume, Wi-Fi, dark mode, login items)
  → Use AppleScript (System Events) + CLI tools (networksetup, defaults, etc.)

Does the app have no AX support?
  (games, Metal/OpenGL apps, some Java/Qt apps)
  → Use Screenshot + OCR + coordinate-based CGEvent clicks

Is it a browser tab?
  → Use AppleScript JS execution (Safari/Chrome) or CDP (Chrome only)
```

### Layered Strategy for computer-pilot

```
Layer 0: Direct CLI/API
  networksetup, defaults, diskutil, security, pmset, shortcuts run, etc.
  Zero UI interaction needed. Most reliable.

Layer 1: AppleScript Semantic
  tell app "Mail" to send message
  tell app "Calendar" to make new event
  Talks to the app's scripting engine. Very reliable.

Layer 2: AX Tree
  Read UI structure, click elements by ref, type into fields.
  Works for any app with AX support. Reliable.

Layer 3: CGEvent (coordinate-based)
  Mouse click at x,y. Keyboard events.
  Works for any app. Less precise (need screenshot to find targets).

Layer 4: Screenshot + OCR + Vision
  When all else fails. Highest latency, highest token cost.
  But universally applicable.
```

---

## Appendix: Complete CLI Tool Reference

| Tool | Path | Purpose |
|------|------|---------|
| `osascript` | /usr/bin/osascript | Run AppleScript or JXA |
| `defaults` | /usr/bin/defaults | Read/write preferences |
| `open` | /usr/bin/open | Open files, apps, URLs |
| `pbcopy` / `pbpaste` | /usr/bin/ | Clipboard |
| `screencapture` | /usr/sbin/screencapture | Screenshots and screen recording |
| `networksetup` | /usr/sbin/networksetup | Network configuration |
| `pmset` | /usr/bin/pmset | Power management |
| `caffeinate` | /usr/bin/caffeinate | Prevent sleep |
| `security` | /usr/bin/security | Keychain operations |
| `diskutil` | /usr/sbin/diskutil | Disk management |
| `hdiutil` | /usr/bin/hdiutil | DMG operations |
| `mdfind` | /usr/bin/mdfind | Spotlight search |
| `mdls` | /usr/bin/mdls | File metadata |
| `mdimport` | /usr/bin/mdimport | Index file in Spotlight |
| `sips` | /usr/bin/sips | Image manipulation |
| `textutil` | /usr/bin/textutil | Document conversion |
| `plutil` | /usr/bin/plutil | Property list manipulation |
| `ditto` | /usr/bin/ditto | Advanced copy with metadata |
| `xattr` | /usr/bin/xattr | Extended attributes |
| `say` | /usr/bin/say | Text to speech |
| `afplay` | /usr/bin/afplay | Play audio |
| `afinfo` | /usr/bin/afinfo | Audio file info |
| `afconvert` | /usr/bin/afconvert | Audio conversion |
| `qlmanage` | /usr/bin/qlmanage | Quick Look |
| `shortcuts` | /usr/bin/shortcuts | Run Shortcuts |
| `automator` | /usr/bin/automator | Run Automator workflows |
| `lsappinfo` | /usr/bin/lsappinfo | Running app info |
| `system_profiler` | /usr/sbin/system_profiler | System information |
| `dscl` | /usr/bin/dscl | Directory services (user mgmt) |
| `sysadminctl` | /usr/sbin/sysadminctl | System admin |
| `launchctl` | /bin/launchctl | Service management |
| `tmutil` | /usr/bin/tmutil | Time Machine |
| `codesign` | /usr/bin/codesign | Code signing |
| `spctl` | /usr/sbin/spctl | Security assessment |
| `log` | /usr/bin/log | Unified logging |
| `lpstat` / `lp` | /usr/bin/ | Print system |

### Homebrew-installable Additions

| Tool | Install | Purpose |
|------|---------|---------|
| `cliclick` | `brew install cliclick` | Mouse/keyboard simulation |
| `blueutil` | `brew install blueutil` | Bluetooth control |
| `tag` | `brew install tag` | Finder tag management |
| `brightness` | `brew install brightness` | Display brightness |
| `displayplacer` | `brew install displayplacer` | Display resolution/arrangement |
| `mas` | `brew install mas` | Mac App Store CLI |
| `terminal-notifier` | `brew install terminal-notifier` | Rich notifications |
| `trash` | `brew install trash` | Move to Trash |
| `imagesnap` | `brew install imagesnap` | Camera capture |
| `ffmpeg` | `brew install ffmpeg` | Audio/video processing |
| `sox` | `brew install sox` | Audio recording |
| `poppler` | `brew install poppler` | PDF tools (pdftotext, etc.) |
| `yabai` | `brew install yabai` | Tiling WM + Space management |
| `jq` | `brew install jq` | JSON processing |
