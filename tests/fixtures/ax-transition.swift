// Untimed AX transition diagnostic for a disposable window, never a benchmark participant.
// swiftc ax-transition.swift -o /tmp/ax-transition
// /tmp/ax-transition PID UNIQUE_WINDOW_TITLE OBSERVED_ROW_LABEL
// Dispatches one AXPress; retains native objects to compare identity across navigation.
import AppKit
import ApplicationServices

func attr(_ e: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(e, name as CFString, &value) == .success ? value : nil
}
func text(_ e: AXUIElement, _ name: String) -> String { attr(e, name) as? String ?? "" }
func label(_ e: AXUIElement) -> String {
    let title = text(e, "AXTitle")
    return title.isEmpty ? text(e, "AXDescription") : title
}
func find(_ e: AXUIElement, role: String, title: String? = nil, depth: Int = 0) -> AXUIElement? {
    if text(e, "AXRole") == role && (title == nil || label(e) == title) { return e }
    if depth >= 50 { return nil }
    for child in attr(e, "AXChildren") as? [AXUIElement] ?? [] {
        if let found = find(child, role: role, title: title, depth: depth + 1) { return found }
    }
    return nil
}
let pid = pid_t(CommandLine.arguments[1])!
let token = CommandLine.arguments[2]
let filename = CommandLine.arguments[3]
let app = AXUIElementCreateApplication(pid)
let window = attr(app, "AXFocusedWindow") as! AXUIElement
precondition(label(window).contains(token), "Not the owned window")
let row = find(window, role: "AXRow", title: filename)!
let web = find(window, role: "AXWebArea")!
let editor = find(window, role: "AXTextArea")!
var events: [[String: Any]] = []
var start = ProcessInfo.processInfo.systemUptime
var observer: AXObserver?
AXObserverCreate(pid, { _, element, notification, _ in
    events.append(["ms": (ProcessInfo.processInfo.systemUptime - start)*1000,
                   "event": notification as String, "role": text(element, "AXRole"), "title": label(element)])
}, &observer)
let names = ["AXValueChanged", "AXSelectedTextChanged", "AXFocusedUIElementChanged", "AXMainWindowChanged", "AXSelectedChildrenChanged", "AXTitleChanged", "AXLayoutChanged"]
var subscribed: [[String:Any]] = []
for (kind, element) in [("app",app), ("window",window), ("webarea",web)] {
    for name in names {
        let result = AXObserverAddNotification(observer!, element, name as CFString, nil)
        subscribed.append(["scope":kind,"name":name,"code":result.rawValue])
    }
}
let source = AXObserverGetRunLoopSource(observer!)
CFRunLoopAddSource(CFRunLoopGetCurrent(), source, .defaultMode)
func state() -> [String:Any] {
    ["ms":(ProcessInfo.processInfo.systemUptime-start)*1000,"window":label(window),"webarea":label(web),"editor":label(editor),"editorValue":text(editor,"AXValue"),"document":text(window,"AXDocument")]
}
let before = state()
start = ProcessInfo.processInfo.systemUptime
let result = AXUIElementPerformAction(row, "AXPress" as CFString)
var samples = [state()]
let deadline = start + 0.6
while ProcessInfo.processInfo.systemUptime < deadline {
    CFRunLoopRunInMode(.defaultMode, 0.02, true)
    let current = state()
    if ["window","webarea","editor","editorValue","document"].contains(where: { String(describing: current[$0]!) != String(describing: samples.last![$0]!) }) {
        samples.append(current)
    }
}
let afterRow = find(window,role:"AXRow",title:filename)!
let afterEditor = find(window,role:"AXTextArea")!
let output:[String:Any] = ["actionCode":result.rawValue,"before":before,"samples":samples,"events":events,"subscriptions":subscribed,"sameRow":CFEqual(row,afterRow),"sameEditor":CFEqual(editor,afterEditor),"afterEditor":label(afterEditor)]
print(String(data:try! JSONSerialization.data(withJSONObject:output,options:[.prettyPrinted,.sortedKeys]),encoding:.utf8)!)
CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .defaultMode)
