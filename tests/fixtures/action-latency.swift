import AppKit

final class Group: NSView {
    override func isAccessibilityElement() -> Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? { .group }
    override func accessibilityChildren() -> [Any]? { subviews }
}
final class Field: NSTextField {
    override func setAccessibilityValue(_ value: Any?) {
        stringValue = value as? String ?? ""
        NSAccessibility.post(element: NSApp!, notification: .valueChanged)
    }
}
final class ReorderingWindow: NSWindow {
    var armed = false
    var reordered = false
    override func accessibilityChildren() -> [Any]? {
        guard var children = super.accessibilityChildren() else { return nil }
        let fields = children.indices.filter { (children[$0] as? NSCell)?.controlView is Field }
        if reordered && fields.count == 2 { children.swapAt(fields[0], fields[1]) }
        // The returned array represents the current read. A later traversal
        // sees the reorder; retaining an AX object must still address it.
        if armed { armed = false; reordered.toggle() }
        return children
    }
}
final class Delegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    var otherWindow: NSWindow?
    var choiceLabel: NSTextField!
    var urgentLabel: NSTextField!
    var transformInput = false
    func applicationDidFinishLaunching(_ notification: Notification) {
        window = ReorderingWindow(contentRect: NSRect(x: 50, y: 360, width: 400, height: 440),
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
        choiceLabel = NSTextField(labelWithString: "Choice: Direct")
        choiceLabel.frame = NSRect(x: 20, y: 300, width: 160, height: 24)
        window.contentView!.addSubview(choiceLabel)
        let popup = NSPopUpButton(frame: NSRect(x: 185, y: 300, width: 170, height: 26))
        popup.addItems(withTitles: ["Direct", "Proxy"])
        popup.setAccessibilityLabel("Route")
        popup.target = self; popup.action = #selector(selectRoute(_:))
        window.contentView!.addSubview(popup)
        let next = NSButton(title: "Next page", target: self, action: #selector(nextPage))
        next.frame = NSRect(x: 270, y: 360, width: 100, height: 26)
        window.contentView!.addSubview(next)
        let urgent = NSButton(checkboxWithTitle: "Urgent", target: self, action: #selector(toggleUrgent(_:)))
        urgent.frame = NSRect(x: 20, y: 420, width: 100, height: 20)
        urgent.state = .on
        window.contentView!.addSubview(urgent)
        urgentLabel = NSTextField(labelWithString: "Urgent callback: untouched")
        urgentLabel.frame = NSRect(x: 120, y: 420, width: 260, height: 20)
        window.contentView!.addSubview(urgentLabel)
        let keyEvents = NSTextField(labelWithString: "Key events: 0")
        keyEvents.frame = NSRect(x: 20, y: 0, width: 160, height: 20)
        window.contentView!.addSubview(keyEvents)
        let arm = NSButton(title: "Reorder on read", target: self, action: #selector(armReorder))
        arm.frame = NSRect(x: 200, y: 0, width: 180, height: 20)
        window.contentView!.addSubview(arm)
        let transform = NSButton(title: "Transform next input", target: self, action: #selector(armTransform))
        transform.frame = NSRect(x: 200, y: 30, width: 180, height: 20)
        window.contentView!.addSubview(transform)
        var keyCount = 0
        NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if self.transformInput {
                self.transformInput = false
                input.stringValue = "changed-by-application"
                NSAccessibility.post(element: NSApp!, notification: .valueChanged)
                return nil
            }
            if event.keyCode == 97 { // F6: delivery completes asynchronously.
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                    input.stringValue = "delayed-key-change"
                    NSAccessibility.post(element: NSApp!, notification: .valueChanged)
                }
                return nil
            }
            if event.keyCode == 98 { // F7: reuse controls for another document.
                self.window.title = "Second document"
                self.window.representedURL = URL(fileURLWithPath: "/tmp/cu-fixture-second-document.txt")
                NSAccessibility.post(element: self.window!, notification: .titleChanged)
                return nil
            }
            if event.keyCode == 36 {
                FileHandle.standardOutput.write(Data("enter handled\n".utf8))
                return nil // A handled shortcut need not change the AX tree.
            }
            keyCount += 1
            keyEvents.stringValue = "Key events: \(keyCount)"
            return event
        }
        window.orderBack(nil)
        window.makeKey()
        window.makeMain()
        window.makeFirstResponder(input)
    }
    @objc func armReorder() {
        (window as? ReorderingWindow)?.armed = true
    }
    @objc func armTransform() { transformInput = true }
    @objc func selectRoute(_ sender: NSPopUpButton) {
        choiceLabel.stringValue = "Choice: \(sender.titleOfSelectedItem!)"
        NSAccessibility.post(element: NSApp!, notification: .valueChanged)
    }
    @objc func toggleUrgent(_ sender: NSButton) {
        urgentLabel.stringValue = "Urgent callback: \(sender.state == .on ? "on" : "off")"
        NSAccessibility.post(element: NSApp!, notification: .valueChanged)
    }
    @objc func nextPage() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [self] in
            window.contentView = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 440))
            let input = Field(frame: NSRect(x: 20, y: 395, width: 350, height: 26))
            input.setAccessibilityIdentifier("ready-input")
            input.stringValue = "Ready for input"
            window.contentView!.addSubview(input)
            let other = NSButton(title: "Other window", target: self, action: #selector(openOtherWindow))
            other.frame = NSRect(x: 20, y: 350, width: 150, height: 26)
            window.contentView!.addSubview(other)
            let deep = NSButton(title: "Deep input", target: self, action: #selector(deepPage))
            deep.frame = NSRect(x: 20, y: 300, width: 150, height: 26)
            window.contentView!.addSubview(deep)
            window.makeFirstResponder(input)
            NSAccessibility.post(element: NSApp!, notification: .layoutChanged)
        }
    }
    @objc func deepPage() {
        window.contentView = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 440))
        var parent = window.contentView!
        // Deliberately beyond the AX traversal limit; focus remains readable.
        for _ in 0..<36 {
            let group = Group(frame: parent.bounds)
            parent.addSubview(group)
            parent = group
        }
        let input = Field(frame: NSRect(x: 20, y: 360, width: 350, height: 26))
        input.stringValue = "deep initial"
        parent.addSubview(input)
        let other = Field(frame: NSRect(x: 20, y: 300, width: 350, height: 26))
        other.stringValue = "deep other"
        parent.addSubview(other)
        NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.characters == "?" { return nil } // Deliberately drop input.
            if event.characters == "!" {
                self.window.makeFirstResponder(other)
                other.stringValue = "focus moved"
                NSAccessibility.post(element: NSApp!, notification: .valueChanged)
                return nil
            }
            return event
        }
        window.makeFirstResponder(input)
    }
    @objc func openOtherWindow() {
        // Same title, geometry, field and value: only window identity differs.
        let original = window.contentView!.subviews.compactMap { $0 as? Field }.first!
        let other = NSWindow(contentRect: window.contentRect(forFrameRect: window.frame),
                             styleMask: [.titled, .closable], backing: .buffered, defer: false)
        other.title = window.title
        let input = Field(frame: original.frame)
        input.setAccessibilityIdentifier("ready-input")
        input.stringValue = original.stringValue
        other.contentView!.addSubview(input)
        otherWindow = other
        other.orderBack(nil)
        other.makeKey()
        other.makeMain()
        other.makeFirstResponder(input)
    }
}
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = Delegate()
app.delegate = delegate
app.run()
