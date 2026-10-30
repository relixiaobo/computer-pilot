// Observe the real Broker panel while a second targeted hover is dispatched.
import Foundation
import CoreGraphics

let binary = CommandLine.arguments[1]
let brokerPID = Int(CommandLine.arguments[2])!
func hover() throws -> (Process, Pipe) {
    let action = Process()
    action.executableURL = URL(fileURLWithPath: binary)
    action.arguments = ["--json", "hover", "777", "499", "--app", "Finder", "--no-snapshot"]
    var environment = ProcessInfo.processInfo.environment
    environment["COMPUTER_PILOT_VISUAL_CURSOR"] = "1"
    action.environment = environment
    let output = Pipe()
    action.standardOutput = output
    action.standardError = output
    try action.run()
    return (action, output)
}
func overlayWindows() -> [[String: Any]] {
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    return windows.filter { $0[kCGWindowOwnerPID as String] as? Int == brokerPID }
}
let (action, output) = try hover()

var positions = Set<Int>()
var unexpectedSize = false
var settled = false
var curved = false
var movingAfterHalfSecond = false
let started = Date()
let deadline = Date().addingTimeInterval(3.0)
while Date() < deadline {
    for window in overlayWindows() {
        guard let bounds = window[kCGWindowBounds as String] as? [String: Double],
              let x = bounds["X"], let y = bounds["Y"],
              let width = bounds["Width"], let height = bounds["Height"] else { continue }
        if x > 317 && x < 716 {
            positions.insert(Int(x.rounded()))
            if Date().timeIntervalSince(started) > 0.5 { movingAfterHalfSecond = true }
        }
        let tipX = x + 60.35
        let tipY = y + 55.7
        if abs((tipY - 299) - (tipX - 377) * 0.5) > 10 { curved = true }
        if width != 126 || height != 126 { unexpectedSize = true }
        if abs(x - 716.65) < 1 && abs(y - 443.3) < 1 && width == 126 && height == 126 {
            settled = true
        }
    }
    Thread.sleep(forTimeInterval: 0.025)
}
action.waitUntilExit()
if action.terminationStatus != 0 {
    print(String(data: output.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? "hover failed")
    exit(1)
}
guard positions.count >= 10, settled, curved, movingAfterHalfSecond, !unexpectedSize else {
    print("motion failed: intermediate positions=\(positions.count), settled=\(settled), curved=\(curved), moving after 0.5s=\(movingAfterHalfSecond), unexpected size=\(unexpectedSize)")
    exit(1)
}
print("motion-ok: \(positions.count) intermediate positions, constant arrow size and tip at (777,499)")

// Verify idle hide and the next event reusing the same Broker/panel,
// without changing user focus.
Thread.sleep(until: started.addingTimeInterval(13))
guard overlayWindows().isEmpty else { print("idle cursor did not hide"); exit(1) }
let (wake, wakeOutput) = try hover()
wake.waitUntilExit()
guard wake.terminationStatus == 0 else {
    print(String(data: wakeOutput.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? "wake hover failed")
    exit(1)
}
let wakeDeadline = Date().addingTimeInterval(1)
while overlayWindows().isEmpty && Date() < wakeDeadline { Thread.sleep(forTimeInterval: 0.025) }
guard !overlayWindows().isEmpty else { print("cursor did not reappear after idle"); exit(1) }
print("idle-wake-ok")
