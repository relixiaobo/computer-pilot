import AppKit

final class Field: NSTextField {
    override func setAccessibilityValue(_ value: Any?) {
        stringValue = value as? String ?? ""
        NSAccessibility.post(element: NSApp!, notification: .valueChanged)
    }
}
final class Delegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(contentRect: NSRect(x: 50, y: 50, width: 400, height: 180),
                          styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Computer Pilot latency fixture"
        let input = Field(frame: NSRect(x: 20, y: 105, width: 350, height: 26))
        input.stringValue = "initial"
        input.setAccessibilityIdentifier("latency-input")
        window.contentView!.addSubview(input)
        let label = NSTextField(labelWithString: "unchanged")
        label.frame = NSRect(x: 20, y: 50, width: 350, height: 25)
        window.contentView!.addSubview(label)
        window.orderBack(nil)
        window.makeKey()
        window.makeMain()
        window.makeFirstResponder(input)
    }
}
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = Delegate()
app.delegate = delegate
app.run()
