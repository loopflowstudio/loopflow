import AppKit
import ApplicationServices

func value(_ e: AXUIElement, _ name: String) -> CFTypeRef? {
    var result: CFTypeRef?
    guard AXUIElementCopyAttributeValue(e, name as CFString, &result) == .success else { return nil }
    return result
}
func text(_ e: AXUIElement, _ key: String) -> String { value(e, key) as? String ?? "" }
func elements(_ root: AXUIElement) -> [AXUIElement] {
    var seen = Set<CFHashCode>(), found = [AXUIElement]()
    func walk(_ e: AXUIElement, _ depth: Int) {
        guard depth < 24, seen.insert(CFHash(e)).inserted else { return }
        found.append(e)
        guard text(e, "AXRole") != "AXMenuBar" else { return }
        for key in ["AXWindows", "AXChildren"] {
            for child in value(e, key) as? [AXUIElement] ?? [] { walk(child, depth + 1) }
        }
    }
    walk(root, 0)
    return found
}
func settle(_ seconds: Double = 0.5) {
    let deadline = Date(timeIntervalSinceNow: seconds)
    while Date() < deadline { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.05)) }
}
struct Failure: Error { let message: String }
func check(_ condition: Bool, _ message: String) throws {
    guard condition else { throw Failure(message: message) }
    print("PASS", message)
}
func find(_ root: AXUIElement, id: String) throws -> AXUIElement {
    guard let e = elements(root).first(where: { text($0, "AXIdentifier") == id }) else { throw Failure(message: "Missing " + id) }
    return e
}
func press(_ root: AXUIElement, title: String) throws {
    guard let e = elements(root).first(where: { text($0,"AXRole") == "AXButton" && (text($0,"AXTitle") == title || text($0,"AXDescription") == title) }) else {
        print("Buttons:", elements(root).filter { text($0,"AXRole") == "AXButton" }.map { text($0,"AXTitle") + text($0,"AXDescription") })
        throw Failure(message: "Missing button " + title)
    }
    try check(AXUIElementPerformAction(e, kAXPressAction as CFString) == .success, "Press " + title)
    settle()
}
let configuration = NSWorkspace.OpenConfiguration()
configuration.createsNewApplicationInstance = true
configuration.activates = true
configuration.arguments = ["--repo", "/Users/jack/src/loopflow.main-view-task"]
var environment = ProcessInfo.processInfo.environment
environment["LF_WAVE_ID"] = "loo291-unrelated-launching-wave"
configuration.environment = environment
let launchedAt = ProcessInfo.processInfo.systemUptime
var finished = false
var exitStatus: Int32 = 0
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-review/Loopflow Review.app"), configuration: configuration) { running, error in
    guard let running else { print("FAIL launch", error?.localizedDescription ?? "unknown"); exitStatus = 1; finished = true; return }
    print("Owned PID", running.processIdentifier)
    DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
        defer { print("Owned termination requested", running.terminate()); finished = true }
        let root = AXUIElementCreateApplication(running.processIdentifier)
        do {
            print("Probe executable", CommandLine.arguments[0], "AX trusted", AXIsProcessTrusted())
            var windows: CFTypeRef?
            let windowStatus = AXUIElementCopyAttributeValue(root, kAXWindowsAttribute as CFString, &windows)
            print("AXWindows", windowStatus.rawValue, "count", (windows as? [AXUIElement])?.count ?? -1)
            let ownedWindows = (CGWindowListCopyWindowInfo(.optionAll, kCGNullWindowID) as? [[String: Any]] ?? []).filter {
                ($0[kCGWindowOwnerPID as String] as? Int32) == running.processIdentifier
            }
            print("Owned onscreen windows", ownedWindows.filter { ($0[kCGWindowIsOnscreen as String] as? Bool) == true }.count,
                  "active", running.isActive, "hidden", running.isHidden)
            try check(AXIsProcessTrusted(), "This exact probe host has Accessibility permission")
            let readyDeadline = ProcessInfo.processInfo.systemUptime + 20
            while !elements(root).contains(where: { text($0,"AXIdentifier") == "workspace-task-ee671927-255f-41e6-8429-b830d59cc1de" }) && ProcessInfo.processInfo.systemUptime < readyDeadline {
                settle(0.05)
            }
            print("TIMING launch_to_observed_task_ms", (ProcessInfo.processInfo.systemUptime - launchedAt) * 1000)
            let initial = elements(root)
            try check(!initial.contains { text($0,"AXIdentifier") == "workspace-planning-unavailable" }, "Ambient Wave does not block planning")
            let tasks = initial.filter { text($0,"AXIdentifier").hasPrefix("workspace-task-") }
            try check(!tasks.isEmpty, "Real Task list loaded (\(tasks.count) rows)")
            let target = try find(root, id: "workspace-task-ee671927-255f-41e6-8429-b830d59cc1de")
            try check(AXUIElementPerformAction(target, kAXPressAction as CFString) == .success, "Open exact LOO-291 Task")
            settle()
            let detail = elements(root).map { text($0,"AXValue") + text($0,"AXTitle") + text($0,"AXDescription") }.joined(separator: "\n")
            try check(detail.contains("No open Sessions for this Work"), "Task has explicit no-Session state")
            try check(detail.contains("In the existing desktop workspace"), "Authoritative directive visible")
            try press(root, title: "Show work list")
            try check(elements(root).contains { text($0,"AXIdentifier").hasPrefix("workspace-task-") }, "List and details visible together")
            try press(root, title: "Hide work list")
            try press(root, title: "All work")
            guard let search = elements(root).first(where: { text($0,"AXRole") == "AXTextField" }) else { throw Failure(message: "Missing search") }
            try check(AXUIElementSetAttributeValue(search, kAXValueAttribute as CFString, "LOO-291" as CFString) == .success, "Set work search")
            settle()
            let filtered = elements(root).filter { text($0,"AXIdentifier").hasPrefix("workspace-task-") }
            try check(filtered.count == 1, "Search reveals the exact Task")
            settle(16)
            try check(text(search,"AXValue") == "LOO-291", "Search survives a planning poll")
            try check(AXUIElementSetAttributeValue(search, kAXValueAttribute as CFString, "" as CFString) == .success, "Clear work search")
            settle()
            try check(AXUIElementSetAttributeValue(search, kAXValueAttribute as CFString, "<lf:skill:design>" as CFString) == .success, "Find existing design Sessions")
            settle()
            guard let elsewhere = elements(root).first(where: {
                text($0,"AXIdentifier").hasPrefix("session-row-") && (text($0,"AXTitle") + text($0,"AXDescription")).contains("ELSEWHERE")
            }) else { throw Failure(message: "No existing elsewhere Session available") }
            let sessionId = text(elsewhere,"AXIdentifier")
            try check(AXUIElementPerformAction(elsewhere, kAXPressAction as CFString) == .success, "Select exact existing " + sessionId)
            settle()
            let explanation = elements(root).map { text($0,"AXValue") + text($0,"AXTitle") + text($0,"AXDescription") }.joined(separator: "\n")
            try check(explanation.contains("Move session here") && explanation.contains("Active in another terminal"), "Existing client requires explicit Move here")
            print("Configured navigation complete; Move here was not invoked and no Session was resolved")
        } catch { print("FAIL", error); exitStatus = 1 }
    }
}
while !finished { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1)) }
exit(exitStatus)
