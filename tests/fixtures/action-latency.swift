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
        window = NSWindow(contentRect: NSRect(x: 50, y: 360, width: 400, height: 440),
                          styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Computer Pilot latency fixture"
        let input = Field(frame: NSRect(x: 20, y: 395, width: 350, height: 26))
        input.stringValue = "initial"
        input.setAccessibilityIdentifier("latency-input")
        window.contentView!.addSubview(input)
        let label = NSTextField(labelWithString: "unchanged")
        label.frame = NSRect(x: 20, y: 360, width: 350, height: 25)
        window.contentView!.addSubview(label)
        // More than 100 elements exercise both action and pre-type coverage.
        for index in 0..<120 {
            let button = NSButton(title: "Item \(index)", target: nil, action: nil)
            button.frame = NSRect(x: 10 + (index % 6) * 64, y: 20 + (index / 6) * 15,
                                  width: 60, height: 12)
            window.contentView!.addSubview(button)
        }
        let other = Field(frame: NSRect(x: 20, y: 330, width: 350, height: 26))
        other.stringValue = "other"
        other.setAccessibilityIdentifier("other-input")
        window.contentView!.addSubview(other)
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
