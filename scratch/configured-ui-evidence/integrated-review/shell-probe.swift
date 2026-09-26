// Single-use review probe; writes only a disposable shell receipt under /tmp.
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
environment["LF_CONTROL_HOME"] = "/Users/jack/.lf-dev/installed/local-be852452823d43d7b7fde663651a7590"
environment["LF_CONTROL_DB_PATH"] = environment["LF_CONTROL_HOME"]! + "/loopflow.db"
environment["LF_HOME"] = environment["LF_CONTROL_HOME"]
environment["LF_CONTROL_BIN"] = "/Users/jack/.local/bin/lf"
configuration.environment = environment
let launchedAt = ProcessInfo.processInfo.systemUptime
var finished = false
var exitStatus: Int32 = 0
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-integration/Loopflow Integration.app"), configuration: configuration) { running, error in
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
            print("TIMING launch_observation_end_ms", (ProcessInfo.processInfo.systemUptime - launchedAt) * 1000)
            let initial = elements(root)
            try check(!initial.contains { text($0,"AXIdentifier") == "workspace-planning-unavailable" }, "Ambient Wave does not block planning")
            let tasks = initial.filter { text($0,"AXIdentifier").hasPrefix("workspace-task-") }
            if tasks.isEmpty { for e in initial { print("EMPTY", text(e,"AXRole"),text(e,"AXIdentifier"),text(e,"AXValue")) } }
            try check(!tasks.isEmpty, "Real Task list loaded (\(tasks.count) rows)")
            let target = try find(root, id: "workspace-task-ee671927-255f-41e6-8429-b830d59cc1de")
            try check(AXUIElementPerformAction(target, kAXPressAction as CFString) == .success, "Open exact LOO-291 Task")
            settle()
            let detail = elements(root).map { text($0,"AXValue") + text($0,"AXTitle") + text($0,"AXDescription") }.joined(separator: "\n")
            try check(detail.contains("No open Sessions for this Work"), "Task has explicit no-Session state")
            try check(detail.contains("In the existing desktop workspace"), "Authoritative directive visible")
            try check(running.isActive, "Owned application is active before input")
            let buttons = elements(root).filter { text($0, "AXRole") == "AXButton" }
            print("BUTTONS", buttons.map { text($0,"AXIdentifier") + " " + text($0,"AXTitle") + text($0,"AXDescription") })
            try check(buttons.contains { (text($0,"AXTitle") + text($0,"AXDescription")).contains("New conversation · LOO-291") }, "Conversation affordance names selected Task")
            try press(root, title: "New terminal")
            settle(2)
            func send(_ input: String, enter: Bool = false) {
                let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: true)!
                let chars = Array(input.utf16)
                chars.withUnsafeBufferPointer { event.keyboardSetUnicodeString(stringLength: chars.count, unicodeString: $0.baseAddress) }
                event.postToPid(running.processIdentifier)
                settle(0.2)
                if enter {
                    CGEvent(keyboardEventSource: nil, virtualKey: 36, keyDown: true)!.postToPid(running.processIdentifier)
                    CGEvent(keyboardEventSource: nil, virtualKey: 36, keyDown: false)!.postToPid(running.processIdentifier)
                    settle(0.2)
                }
            }
            let receipt = "/tmp/loo291-integrated-review-input-" + String(running.processIdentifier)
            send("stty -echo; cat > " + receipt, enter: true)
            settle(1)
            let prefix = "Reply with only the "
            let suffix = "continuity word from my first message."
            // Same bulk event and pause as the provider proof, first without navigation.
            send(prefix)
            send(suffix, enter: true)
            settle(0.5)
            let baseline = try String(contentsOfFile: receipt, encoding: .utf8)
            print("NO_NAVIGATION_BYTES", baseline.debugDescription)
            send(prefix)
            try press(root, title: "Show work list")
            try press(root, title: "Work details")
            try press(root, title: "All work")
            try press(root, title: "Return to terminals")
            send(suffix, enter: true)
            settle(0.5)
            let retained = try String(contentsOfFile: receipt, encoding: .utf8)
            print("AFTER_NAVIGATION_BYTES", retained.debugDescription)
            try check(baseline == prefix + suffix + "\n", "Exact bulk input reaches shell before navigation")
            try check(retained == baseline + baseline, "Exact shell draft survives A/D and details/overview")
        } catch { print("FAIL", error); exitStatus = 1 }
    }
}
while !finished { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1)) }
exit(exitStatus)
