import AppKit

// Disposable, non-scriptable order entry UI for the long agent benchmark.
// The agent sees only the UI; the harness independently checks the saved file.
final class OrderField: NSTextField {
    override func setAccessibilityValue(_ value: Any?) {
        stringValue = value as? String ?? ""
        NSAccessibility.post(element: NSApp!, notification: .valueChanged)
    }
}

final class OrderWorkflow: NSObject, NSApplicationDelegate {
    let keys = ["Customer", "SKU", "Quantity", "City", "Memo"]
    let source: [[String: String]]
    let output: URL
    var records: [[String: String]] = []
    var initial: [[String: String]] = []
    var edits: [Int] = []
    var index = 0
    var editing = false
    var completed = false
    var window: NSWindow!
    var fields: [String: OrderField] = [:]
    var priority: NSButton!
    var priorityLabel: NSTextField!

    init(input: String, output: String) throws {
        source = try JSONDecoder().decode([[String: String]].self,
                                         from: Data(contentsOf: URL(fileURLWithPath: input)))
        self.output = URL(fileURLWithPath: output)
        super.init()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(contentRect: NSRect(x: 40, y: 180, width: 650, height: 660),
                          styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Computer Pilot order workflow"
        showForm()
        window.orderBack(nil)
        window.makeKey()
        window.makeMain()
    }

    func resetView() {
        window.contentView = NSView(frame: NSRect(x: 0, y: 0, width: 650, height: 660))
        fields.removeAll()
    }

    func label(_ text: String, y: Int, height: Int = 24) {
        let label = NSTextField(wrappingLabelWithString: text)
        label.frame = NSRect(x: 20, y: y, width: 610, height: height)
        window.contentView!.addSubview(label)
    }

    func button(_ title: String, y: Int, action: Selector, tag: Int = 0) {
        let button = NSButton(title: title, target: self, action: action)
        button.frame = NSRect(x: 20, y: y, width: 220, height: 30)
        button.tag = tag
        window.contentView!.addSubview(button)
    }

    func showForm() {
        resetView()
        label("\(editing ? "Correct" : "Enter") order \(index + 1) of \(source.count)", y: 625)
        for (n, key) in keys.enumerated() {
            label("Source \(key): \(source[index][key]!)", y: 575 - n * 38, height: 36)
        }
        for (n, key) in keys.enumerated() {
            label(key, y: 365 - n * 51)
            let field = OrderField(frame: NSRect(x: 130, y: 362 - n * 51, width: 490, height: 28))
            field.setAccessibilityIdentifier(key)
            field.stringValue = editing ? records[index][key, default: ""] : ""
            window.contentView!.addSubview(field)
            fields[key] = field
        }
        priority = NSButton(checkboxWithTitle: "Priority", target: self, action: #selector(priorityChanged))
        priority.frame = NSRect(x: 20, y: 100, width: 200, height: 28)
        priority.state = editing && records[index]["Priority"] == "true" ? .on : .off
        window.contentView!.addSubview(priority)
        priorityLabel = NSTextField(labelWithString: "")
        priorityLabel.frame = NSRect(x: 250, y: 100, width: 280, height: 28)
        window.contentView!.addSubview(priorityLabel)
        priorityChanged()
        button(editing ? "Save correction" : "Save and next", y: 45, action: #selector(save))
        window.makeFirstResponder(fields["Customer"])
        NSAccessibility.post(element: NSApp!, notification: .layoutChanged)
    }

    @objc func priorityChanged() {
        priorityLabel.stringValue = "Priority status: \(priority.state == .on ? "on" : "off")"
    }

    @objc func save() {
        var record = Dictionary(uniqueKeysWithValues: keys.map { ($0, fields[$0]!.stringValue) })
        record["Priority"] = priority.state == .on ? "true" : "false"
        if editing {
            records[index] = record
            edits.append(index + 1)
            editing = false
            showReview()
        } else {
            records.append(record)
            initial.append(record)
            index += 1
            if index == source.count { showReview() } else { showForm() }
        }
        persist()
    }

    func showReview() {
        resetView()
        label("Review \(records.count) saved orders", y: 625)
        for (n, record) in records.enumerated() {
            label("Order \(n + 1): \(record["Customer"]!) | \(record["City"]!) | qty \(record["Quantity"]!) | priority \(record["Priority"]!)",
                  y: 570 - n * 75, height: 32)
            button("Edit order \(n + 1)", y: 540 - n * 75, action: #selector(edit(_:)), tag: n)
        }
        button("Finish archive", y: 45, action: #selector(finish))
        NSAccessibility.post(element: NSApp!, notification: .layoutChanged)
    }

    @objc func edit(_ sender: NSButton) {
        index = sender.tag
        editing = true
        showForm()
    }

    @objc func finish() {
        completed = true
        resetView()
        label("Archive complete: \(records.count) orders", y: 580)
        persist()
        NSAccessibility.post(element: NSApp!, notification: .layoutChanged)
    }

    func persist() {
        let data: [String: Any] = ["records": records, "initial": initial,
                                   "edits": edits, "completed": completed]
        do {
            try JSONSerialization.data(withJSONObject: data, options: [.prettyPrinted, .sortedKeys])
                .write(to: output, options: .atomic)
        } catch {
            label("Save failed: \(error)", y: 10)
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let workflow = try OrderWorkflow(input: CommandLine.arguments[1], output: CommandLine.arguments[2])
app.delegate = workflow
app.run()
