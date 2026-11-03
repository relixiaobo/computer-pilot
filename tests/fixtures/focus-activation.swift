import AppKit

// Independent, fresh-process observation for the command test and cleanup.
if CommandLine.arguments.count > 1 {
    if CommandLine.arguments[1] == "--restore", CommandLine.arguments.count == 3,
       let pid = Int32(CommandLine.arguments[2]),
       let previous = NSRunningApplication(processIdentifier: pid) {
        _ = previous.activate(options: [])
        let deadline = Date().addingTimeInterval(1.5)
        while NSWorkspace.shared.frontmostApplication?.processIdentifier != pid && Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.025))
        }
    }
    let pointer = NSEvent.mouseLocation
    let result: [String: Any] = ["pid": NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0,
                               "x": pointer.x, "y": pointer.y]
    print(String(data: try! JSONSerialization.data(withJSONObject: result), encoding: .utf8)!)
    exit(0)
}

final class FixtureApplication: NSApplication {
    // Accepted AX setters/raise must not stand in for actual app activation.
    // This forces cu to exercise its native PID activation and verification.
    override func setAccessibilityFrontmost(_ value: Bool) {}
}
final class FixtureWindow: NSWindow {
    override func accessibilityPerformRaise() -> Bool { true }
    override func setAccessibilityMain(_ value: Bool) {}
}
final class Delegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    func applicationDidFinishLaunching(_ notification: Notification) {
        window = FixtureWindow(contentRect: NSRect(x: 100, y: 300, width: 350, height: 150),
                               styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = ProcessInfo.processInfo.environment["FOCUS_FIXTURE_TITLE"]!
        let field = NSTextField(frame: NSRect(x: 20, y: 50, width: 300, height: 30))
        field.stringValue = "focus fixture"
        window.contentView!.addSubview(field)
        window.orderBack(nil)
        window.makeKey()
        window.makeMain()
        window.makeFirstResponder(field)
    }
}
let app = FixtureApplication.shared
app.setActivationPolicy(.regular)
let delegate = Delegate()
app.delegate = delegate
app.run()
