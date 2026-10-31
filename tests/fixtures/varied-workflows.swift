import AppKit

// Three disposable native workflows. Expected answers live only in the verifier.
final class BenchField: NSTextField {
    override func setAccessibilityValue(_ value: Any?) {
        stringValue = value as? String ?? ""
        NSAccessibility.post(element: NSApp!, notification: .valueChanged)
    }
}
final class BenchEditor: NSTextView {
    override func setAccessibilityValue(_ value: Any?) {
        string = value as? String ?? ""
        NSAccessibility.post(element: NSApp!, notification: .valueChanged)
    }
}
final class Workflows: NSObject, NSApplicationDelegate {
    let mode: String
    let items: [[String: String]]
    let output: URL
    var window: NSWindow!
    var fields: [String: BenchField] = [:]
    var checks: [String: NSButton] = [:]
    var statusLabels: [String: NSTextField] = [:]
    var editor: BenchEditor?
    var popup: NSPopUpButton?
    var records: [String: [String: String]] = [:]
    var events: [[String: String]] = []
    var order: [String]
    var selected = 0
    var page = 0
    var filter = "All"
    var query = ""
    var draft: [String: String] = [:]
    var stage = "list"
    var completed = false
    let width: CGFloat = 700
    let height: CGFloat = 730

    init(input: String, output: String) throws {
        let data = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: input))) as! [String: Any]
        mode = data["mode"] as! String
        items = data["items"] as! [[String: String]]
        order = items.map { $0["ID"]! }
        self.output = URL(fileURLWithPath: output)
        super.init()
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(contentRect: NSRect(x: 60, y: 130, width: width, height: height),
                          styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Computer Pilot \(mode) benchmark"
        showList()
        window.orderBack(nil); window.makeKey(); window.makeMain()
    }
    func clear(_ title: String) {
        fields.removeAll(); checks.removeAll(); statusLabels.removeAll(); editor = nil; popup = nil
        window.contentView = NSView(frame: NSRect(x: 0, y: 0, width: width, height: height))
        label(title, top: 15, h: 35)
    }
    func notify() { NSAccessibility.post(element: NSApp!, notification: .layoutChanged) }
    @discardableResult func label(_ text: String, top: CGFloat, h: CGFloat = 28, x: CGFloat = 20, w: CGFloat = 650) -> NSTextField {
        let view = NSTextField(wrappingLabelWithString: text)
        view.frame = NSRect(x: x, y: height - top - h, width: w, height: h)
        window.contentView!.addSubview(view)
        return view
    }
    func button(_ title: String, _ action: String, top: CGFloat, x: CGFloat = 20, w: CGFloat = 210) {
        let view = NSButton(title: title, target: self, action: #selector(clicked(_:)))
        view.identifier = NSUserInterfaceItemIdentifier(action)
        view.frame = NSRect(x: x, y: height - top - 32, width: w, height: 32)
        window.contentView!.addSubview(view)
    }
    func field(_ key: String, _ value: String, top: CGFloat) {
        label(key, top: top, w: 145)
        let view = BenchField(frame: NSRect(x: 175, y: height - top - 30, width: 500, height: 30))
        view.stringValue = value; view.setAccessibilityIdentifier(key)
        window.contentView!.addSubview(view); fields[key] = view
    }
    func check(_ key: String, _ enabled: Bool, top: CGFloat) {
        let view = NSButton(checkboxWithTitle: key, target: self, action: #selector(checked(_:)))
        view.identifier = NSUserInterfaceItemIdentifier(key)
        view.state = enabled ? .on : .off
        view.frame = NSRect(x: 20, y: height - top - 30, width: 250, height: 30)
        window.contentView!.addSubview(view); checks[key] = view
        statusLabels[key] = label("\(key) status: \(enabled ? "on" : "off")", top: top, x: 290, w: 370)
    }
    @objc func checked(_ sender: NSButton) {
        let key = sender.identifier!.rawValue
        statusLabels[key]?.stringValue = "\(key) status: \(sender.state == .on ? "on" : "off")"
        NSAccessibility.post(element: NSApp!, notification: .valueChanged)
    }
    func collect() -> [String: String] {
        var result = draft
        for (key, field) in fields { result[key] = field.stringValue }
        for (key, box) in checks { result[key] = box.state == .on ? "true" : "false" }
        if let editor = editor { result["Body"] = editor.string }
        if let popup = popup { result["Route"] = popup.titleOfSelectedItem ?? "Direct" }
        return result
    }
    func event(_ action: String, _ extra: [String: String] = [:]) {
        var entry = extra; entry["action"] = action; entry["ID"] = items[selected]["ID"]!
        events.append(entry)
    }
    func showList() {
        stage = "list"
        clear(mode == "triage" ? "Ticket triage" : mode == "document" ? "Document sections" : "Deployment profiles")
        if completed { label("Workflow complete: \(mode)", top: 65); notify(); return }
        if mode == "triage" {
            label("Filter: \(filter) | Search: \(query.isEmpty ? "none" : query)", top: 60)
            button("Open tickets", "filter", top: 95)
            field("Search ID", query, top: 140)
            button("Search", "search", top: 180)
            let visible = items.enumerated().filter { (_, item) in
                (filter == "All" || item["Status"] == filter) && (query.isEmpty || item["ID"]!.contains(query))
            }
            let pageItems = Array(visible.dropFirst(page * 3).prefix(3))
            for (row, pair) in pageItems.enumerated() {
                let (index, item) = pair
                let top = CGFloat(240 + row * 110)
                label("\(item["ID"]!) | \(item["Customer"]!) | \(item["Status"]!) | \(records[item["ID"]!] == nil ? "unassigned" : "saved")", top: top)
                button("Open \(item["ID"]!)", "open:\(index)", top: top + 35)
            }
            if page > 0 { button("Previous page", "previous", top: 590) }
            if (page + 1) * 3 < visible.count { button("Next page", "next", top: 590, x: 250) }
        } else {
            for (row, id) in order.enumerated() {
                let index = items.firstIndex { $0["ID"] == id }!
                let top = CGFloat(65 + row * 82)
                label("\(row + 1). \(id) | \(items[index]["Title"]!) | \(records[id] == nil ? "pending" : "saved")", top: top)
                button("Open \(id)", "open:\(index)", top: top + 30)
                if mode == "document" { button("Move \(id)", "move:\(index)", top: top + 30, x: 260) }
            }
        }
        button("Finish \(mode)", "finish", top: 660)
        notify()
    }
    func showTriage() {
        stage = "edit"; let item = items[selected]
        clear("Ticket \(item["ID"]!)")
        for (i, key) in ["Customer", "Severity", "Plan", "Region", "Status"].enumerated() {
            label("Source \(key): \(item[key]!)", top: CGFloat(65 + i * 40))
        }
        field("Queue", draft["Queue", default: "Unassigned"], top: 290)
        field("SLA hours", draft["SLA hours", default: "0"], top: 340)
        field("Note", draft["Note", default: ""], top: 390)
        check("Urgent", draft["Urgent", default: item["Initial urgent"]!] == "true", top: 450)
        button("Save assignment", "save-ticket", top: 570)
        button("Back to tickets", "list", top: 620)
        notify()
    }
    func showDocument() {
        stage = "edit"; let item = items[selected]
        clear("Section \(item["ID"]!)")
        label("Source heading: \(item["Title"]!)", top: 65)
        label("Source text: \(item["Text"]!)", top: 110, h: 140)
        field("Heading", draft["Heading", default: ""], top: 270)
        label("Body", top: 320)
        let scroll = NSScrollView(frame: NSRect(x: 20, y: height - 360 - 160, width: 650, height: 160))
        scroll.hasVerticalScroller = true
        let view = BenchEditor(frame: scroll.bounds)
        view.isRichText = false; view.isVerticallyResizable = true
        view.autoresizingMask = [.width]; view.string = draft["Body", default: ""]
        view.setAccessibilityLabel("Body"); view.setAccessibilityIdentifier("Body")
        scroll.documentView = view; window.contentView!.addSubview(scroll); editor = view
        check("Reviewed", draft["Reviewed", default: "false"] == "true", top: 550)
        button("Save section", "save-section", top: 615)
        notify()
    }
    func showNetwork() {
        stage = "network"; let item = items[selected]
        clear("\(item["ID"]!) — Network step")
        for (i, key) in ["Title", "Environment", "Route", "Host", "Proxy"].enumerated() {
            label("Source \(key): \(item[key]!)", top: CGFloat(60 + i * 38))
        }
        field("Host", draft["Host", default: ""], top: 270)
        label("Route", top: 320)
        let view = NSPopUpButton(frame: NSRect(x: 175, y: height - 320 - 32, width: 250, height: 32), pullsDown: false)
        view.addItems(withTitles: ["Direct", "Proxy"])
        view.selectItem(withTitle: draft["Route", default: "Direct"])
        view.setAccessibilityLabel("Route")
        view.target = self; view.action = #selector(routeChanged)
        window.contentView!.addSubview(view); popup = view
        if view.titleOfSelectedItem == "Proxy" { field("Proxy server", draft["Proxy server", default: ""], top: 370) }
        field("Port", draft["Port", default: "0"], top: 420)
        check("TLS", draft["TLS", default: "false"] == "true", top: 470)
        button("Next: policy", "policy", top: 610)
        notify()
    }
    @objc func routeChanged() { draft = collect(); showNetwork() }
    func showPolicy() {
        stage = "policy"; let item = items[selected]
        clear("\(item["ID"]!) — Policy step")
        for (i, key) in ["Environment", "Limit", "Tag"].enumerated() {
            label("Source \(key): \(item[key]!)", top: CGFloat(75 + i * 45))
        }
        field("Retention days", draft["Retention days", default: "0"], top: 260)
        field("Limit", draft["Limit", default: "0"], top: 315)
        field("Tag", draft["Tag", default: ""], top: 370)
        check("Audit", draft["Audit", default: "false"] == "true", top: 430)
        button("Next: review", "review", top: 550)
        button("Back: network", "network", top: 610)
        notify()
    }
    func showReview() {
        stage = "review"; clear("\(items[selected]["ID"]!) — Review step")
        for (i, key) in draft.keys.sorted().enumerated() { label("\(key): \(draft[key]!)", top: CGFloat(65 + i * 40)) }
        button("Back: policy", "back-policy", top: 570)
        button("Apply profile", "confirm-profile", top: 630)
        notify()
    }
    func confirm(_ title: String, action: @escaping () -> Void) {
        let alert = NSAlert(); alert.messageText = title
        alert.informativeText = "This writes only to the disposable benchmark workspace."
        alert.addButton(withTitle: "Confirm"); alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: window) { response in
            if response == .alertFirstButtonReturn { action() }
        }
        notify()
    }
    @objc func clicked(_ sender: NSButton) {
        let action = sender.identifier!.rawValue
        if action.hasPrefix("open:") {
            selected = Int(action.split(separator: ":")[1])!; draft = records[items[selected]["ID"]!] ?? [:]
            event("open")
            if mode == "triage" { showTriage() } else if mode == "document" { showDocument() } else { showNetwork() }
        } else if action.hasPrefix("move:") {
            selected = Int(action.split(separator: ":")[1])!; stage = "move"
            clear("Move \(items[selected]["ID"]!)")
            field("New position", "", top: 120)
            button("Move section", "move-save", top: 200); notify()
        } else {
            switch action {
            case "filter": filter = "Open"; query = ""; page = 0; event("filter-open"); showList()
            case "search": query = fields["Search ID"]!.stringValue; page = 0; event("search", ["query": query]); showList()
            case "next": page += 1; showList()
            case "previous": page -= 1; showList()
            case "list": showList()
            case "save-ticket", "save-section":
                records[items[selected]["ID"]!] = collect(); event("save", records[items[selected]["ID"]!]!); persist(); showList()
            case "move-save":
                if let position = Int(fields["New position"]!.stringValue), (1...order.count).contains(position) {
                    let id = items[selected]["ID"]!; order.removeAll { $0 == id }; order.insert(id, at: position - 1)
                    event("move", ["position": String(position)]); persist(); showList()
                }
            case "network": draft = collect(); showNetwork()
            case "policy": draft = collect(); event("network-saved", draft); showPolicy()
            case "review": draft = collect(); event("policy-saved", draft); showReview()
            case "back-policy": event("back-policy"); showPolicy()
            case "confirm-profile":
                confirm("Apply \(items[selected]["ID"]!)?") { [self] in
                    event("apply-confirmed", draft); stage = "saving"; clear("Saving profile…"); notify()
                    let id = items[selected]["ID"]!; let saved = draft
                    let delay = [0.35, 0.85, 0.45, 0.95][selected]
                    DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [self] in
                        records[id] = saved; event("applied", saved); persist(); showList()
                    }
                }
            case "finish":
                confirm("Finish \(mode)?") { [self] in completed = true; event("finished"); persist(); showList() }
            default: break
            }
        }
    }
    func persist() {
        let value: [String: Any] = ["mode": mode, "records": records, "order": order, "events": events, "completed": completed]
        do { try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]).write(to: output, options: .atomic) }
        catch { label("Save failed: \(error)", top: 700) }
    }
}
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let workflow = try Workflows(input: CommandLine.arguments[1], output: CommandLine.arguments[2])
app.delegate = workflow
app.run()
