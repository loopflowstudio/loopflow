// Historical, single-use proof: the hardcoded test-owned Session is now completed.
// Exact draft assertion below was strengthened after inspecting the receipt; not rerun.
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
            if tasks.isEmpty { for e in initial { print("EMPTY",text(e,"AXRole"),text(e,"AXIdentifier"),text(e,"AXTitle"),text(e,"AXDescription"),String(text(e,"AXValue").prefix(240))) } }
            try check(!tasks.isEmpty, "Real Task list loaded (\(tasks.count) rows)")
            let taskId = "workspace-task-ee671927-255f-41e6-8429-b830d59cc1de"
            let target = try find(root, id: taskId)
            try check(AXUIElementPerformAction(target, kAXPressAction as CFString) == .success, "Open Task for owned Session")
            settle()
            let ownedId = "run_edb7d3ad4ac94ddcbaeca8679503845f"
            // The Task has exactly one shared Session, verified in the CLI receipt.
            // SwiftUI inherits the surface identifier here; use its actual AX label.
            try press(root, title: "<lf:wave name=\"product\">")
            settle(8)
            try check(running.activate(options: [.activateAllWindows]), "Activate owned proof window")
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
            func clientReceipt() throws -> String {
                let dir = URL(fileURLWithPath: "/Users/jack/.lf/runs/ed/" + ownedId + "/provider-clients")
                let files = try FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil).filter { file in
                    guard file.pathExtension == "json", let data = try? Data(contentsOf: file),
                          let receipt = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                          let pid = receipt["pid"] as? Int32 else { return false }
                    return kill(pid, 0) == 0
                }
                try check(files.count == 1, "Exactly one test-owned provider client")
                return try String(contentsOf: files[0], encoding: .utf8)
            }
            let originalClient = try clientReceipt()
            print("Provider client before navigation", originalClient)
            send("Reply with only the ")
            try press(root, title: "Show work list")
            try press(root, title: "Work details")
            try press(root, title: "All work")
            let returnTarget = try find(root, id: taskId)
            try check(AXUIElementPerformAction(returnTarget, kAXPressAction as CFString) == .success, "Return to exact Task")
            settle()
            try press(root, title: "<lf:wave name=\"product\">")
            try press(root, title: "New shell")
            settle(2)
            send("printf '%s\\n' \"$$\" > /tmp/loo291-companion-owned.pid; while IFS= read -r proof_line; do printf '%s:%s\\n' \"$$\" \"$proof_line\" >> /tmp/loo291-companion-replies.log; done", enter: true)
            settle(1)
            let companionPid = try String(contentsOfFile: "/tmp/loo291-companion-owned.pid", encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
            print("Companion PID", companionPid)
            try press(root, title: "Work details")
            try press(root, title: "<lf:wave name=\"product\">")
            try check(try clientReceipt() == originalClient, "Navigation and companion split retain exact provider PID/birth")
            send("continuity word from my first message.", enter: true)
            let historyPath = "/Users/jack/.lf/accounts/codex/engineering/sessions/2026/09/23/rollout-2026-09-23T18-18-53-01a0d0fe-7860-7393-ab3d-d6e687a7ea22.jsonl"
            let deadline = ProcessInfo.processInfo.systemUptime + 30
            var replies = 0
            while ProcessInfo.processInfo.systemUptime < deadline {
                let history = try String(contentsOfFile: historyPath, encoding: .utf8)
                replies = history.split(separator: "\n").filter { line in
                    guard let data = String(line).data(using: .utf8),
                          let row = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                          row["type"] as? String == "response_item",
                          let payload = row["payload"] as? [String: Any], payload["role"] as? String == "assistant",
                          let content = payload["content"] as? [[String: Any]] else { return false }
                    return content.contains { $0["text"] as? String == "marigold" }
                }.count
                if replies >= 2 { break }; settle(0.2)
            }
            try check(replies >= 2, "Provider replied after navigation")
            let history = try String(contentsOfFile: historyPath, encoding: .utf8)
            let userMessages = history.split(separator: "\n").compactMap { line -> String? in
                guard let data = String(line).data(using: .utf8),
                      let row = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                      row["type"] as? String == "response_item",
                      let payload = row["payload"] as? [String: Any], payload["role"] as? String == "user",
                      let content = payload["content"] as? [[String: Any]] else { return nil }
                return content.compactMap { $0["text"] as? String }.joined()
            }
            try check(userMessages.last == "Reply with only the continuity word from my first message.",
                      "Exact unfinished draft survives navigation")
            try press(root, title: "Complete session")
            settle(1)
            try check(!elements(root).contains { text($0,"AXIdentifier") == "session-action-complete" }, "Resolved Session pane removed")
            send("AFTER_COMPLETE", enter: true)
            settle(1)
            let companionReply = try String(contentsOfFile: "/tmp/loo291-companion-replies.log", encoding: .utf8)
            try check(companionReply.contains(companionPid + ":AFTER_COMPLETE"), "Same companion process responds after Session completion")
            try press(root, title: "Work details")
            try check(elements(root).contains { text($0,"AXValue").contains("No open Sessions for this Work") }, "Task survives completion with explicit no-Session state")
            print("Configured retained draft, companion and completion proof passed")
        } catch { print("FAIL", error); exitStatus = 1 }
    }
}
while !finished { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1)) }
exit(exitStatus)
