import AppKit

if CommandLine.arguments.contains("--observe") {
    let pointer = NSEvent.mouseLocation
    let result: [String: Any] = ["pid": NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0,
                               "x": pointer.x, "y": pointer.y]
    print(String(data: try! JSONSerialization.data(withJSONObject: result), encoding: .utf8)!)
    exit(0)
}
final class Delegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(contentRect: NSRect(x: 100, y: 300, width: 350, height: 150),
                          styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Computer Pilot launch identity fixture"
        window.orderBack(nil)
        window.makeMain()
    }
}
if let delay = Bundle.main.object(forInfoDictionaryKey: "CPLaunchDelay") as? Double {
    Thread.sleep(forTimeInterval: delay)
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = Delegate()
app.delegate = delegate
app.run()
