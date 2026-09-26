// Read-only configured Task Monitor; never opens, transfers or resolves a Session.
import AppKit
import ApplicationServices

func value(_ element: AXUIElement, _ key: String) -> CFTypeRef? {
    var result: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, key as CFString, &result) == .success else { return nil }
    return result
}
func elements(_ root: AXUIElement) -> [AXUIElement] {
    var seen: [AXUIElement] = []
    func walk(_ node: AXUIElement, _ depth: Int) -> [AXUIElement] {
        guard depth < 24, !seen.contains(where: { CFEqual($0, node) }) else { return [] }
        seen.append(node)
        guard value(node, "AXRole") as? String != "AXMenuBar" else { return [] }
        return [node] + ["AXWindows", "AXChildren"].flatMap { key in
            (value(node, key) as? [AXUIElement] ?? []).flatMap { walk($0, depth + 1) }
        }
    }
    return walk(root, 0)
}
func find(_ root: AXUIElement, _ id: String) -> AXUIElement? {
    let deadline = Date(timeIntervalSinceNow: 12)
    repeat {
        if let node = elements(root).first(where: { value($0, "AXIdentifier") as? String == id }) { return node }
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
    } while Date() < deadline
    return nil
}
setbuf(stdout, nil)
print("Observed at", ISO8601DateFormatter().string(from: Date()))
let console = CGSessionCopyCurrentDictionary() as? [String: Any]
guard AXIsProcessTrusted(), (console?["CGSSessionScreenIsLocked"] as? NSNumber)?.boolValue != true else {
    print("UNAVAILABLE: exact runner lacks AX access or desktop is locked"); exit(2)
}
let repo = "/Users/jack/src/loopflow.main-view-task"
let cli = repo + "/target/debug/lf"
var environment = ProcessInfo.processInfo.environment
for key in ["LF_RUN_ID", "LF_RUN_DIR", "LF_PARENT_RUN_ID", "LF_PROCESS_ID", "LF_WORK_ADVANCE_CLAIM", "LF_WAVE_ID", "LF_HUMAN_SESSION", "LF_HUMAN_SESSION_RUN", "LOOPFLOW_UI_TEST_MODE", "LOOPFLOW_UI_SNAPSHOT_PATH"] {
    environment.removeValue(forKey: key)
}
guard let home = environment["LOO291_REVIEW_HOME"] else {
    print("UNAVAILABLE: select the existing development Home with LOO291_REVIEW_HOME"); exit(2)
}
environment["LF_HOME"] = home
environment["LF_CONTROL_HOME"] = home
environment["LF_DB_PATH"] = home + "/loopflow.db"
environment["LF_CONTROL_DB_PATH"] = home + "/loopflow.db"
func query(_ arguments: [String]) throws -> Any {
    let process = Process(), output = Pipe()
    process.executableURL = URL(fileURLWithPath: cli)
    process.arguments = arguments
    process.currentDirectoryURL = URL(fileURLWithPath: repo)
    process.environment = environment
    process.standardOutput = output
    process.standardError = FileHandle.standardError
    try process.run()
    let data = output.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { throw NSError(domain: "ConfiguredRead", code: Int(process.terminationStatus)) }
    return try JSONSerialization.jsonObject(with: data)
}
func sessionIDs() throws -> Set<String> {
    Set((try query(["session", "list", "--json"]) as! [[String: Any]]).compactMap { $0["id"] as? String })
}
let before: Set<String>
let roadmap: [String: Any]
do {
    before = try sessionIDs()
    roadmap = try query(["roadmap", "--all", "--json"]) as! [String: Any]
} catch { print("UNAVAILABLE configured read", error); exit(2) }
let tasks = (roadmap["waves"] as! [[String: Any]]).flatMap { wave in
    ((wave["projects"] as? [String: Any])?["items"] as? [[String: Any]] ?? []).flatMap { $0["tasks"] as? [[String: Any]] ?? [] }
}
guard let target = tasks.first(where: { ($0["task"] as? [String: Any])?["identifier"] as? String == "LOO-291" }),
      let planning = target["task"] as? [String: Any], let taskID = planning["id"] as? String else {
    print("UNAVAILABLE: LOO-291 absent from configured planning"); exit(2)
}
let configuration = NSWorkspace.OpenConfiguration()
configuration.createsNewApplicationInstance = true
configuration.activates = true
configuration.arguments = ["--repo", repo]
configuration.environment = environment
var finished = false, succeeded = false
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-monitor-review/Loopflow Monitor.app"), configuration: configuration) { app, error in
    guard let app else { print("FAIL launch", error?.localizedDescription ?? "unknown"); finished = true; return }
    print("Owned app PID", app.processIdentifier, "Home", home)
    DispatchQueue.main.async {
        defer { print("Owned app termination requested", app.terminate()); finished = true }
        let root = AXUIElementCreateApplication(app.processIdentifier)
        guard let search = find(root, "workspace-search") ?? elements(root).first(where: { value($0, "AXRole") as? String == "AXTextField" }) else {
            print("FAIL no outline search; active", app.isActive)
            print("Window roles", (value(root, "AXWindows") as? [AXUIElement] ?? []).map { value($0, "AXRole") as? String ?? "unknown" })
            print("Available identifiers", elements(root).compactMap { value($0, "AXIdentifier") as? String })
            return
        }
        guard AXUIElementSetAttributeValue(search, "AXValue" as CFString, "LOO-291" as CFString) == .success else { print("FAIL search input"); return }
        guard let task = find(root, "workspace-task-" + taskID),
              AXUIElementPerformAction(task, kAXPressAction as CFString) == .success,
              find(root, "task-monitor-" + taskID) != nil else { print("FAIL exact Task Monitor selection"); return }
        print("PASS configured Monitor selected by planning ID", taskID)
        guard let refresh = find(root, "monitor-refresh-" + taskID),
              AXUIElementPerformAction(refresh, kAXPressAction as CFString) == .success else {
            print("FAIL Monitor refresh control"); return
        }
        func texts() -> [String] {
            elements(root).flatMap { element in
                ["AXValue", "AXTitle", "AXDescription"].compactMap { value(element, $0) as? String }
            }
        }
        let observedDeadline = Date(timeIntervalSinceNow: 12)
        while !texts().contains(where: { $0.hasPrefix("Observed ") }), Date() < observedDeadline {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
        }
        guard texts().contains(where: { $0.hasPrefix("Observed ") }) else {
            print("FAIL no observation time", texts()); return
        }
        print("PASS observation time rendered")
        let observed = texts().filter { $0.hasPrefix("Observed ") || $0.contains("No active Runs") || $0.contains("No matching Runs") || $0.contains("unavailable") }
        print("Monitor evidence", observed)
        guard let inspect = elements(root).first(where: { element in
            value(element, "AXRole") as? String == "AXButton" &&
                ["AXTitle", "AXDescription"].contains(where: { value(element, $0) as? String == "Inspect" })
        }), AXUIElementPerformAction(inspect, kAXPressAction as CFString) == .success,
            find(root, "workspace-task-directive") != nil else { print("FAIL Inspect"); return }
        guard let monitor = find(root, "task-show-monitor-" + taskID),
              AXUIElementPerformAction(monitor, kAXPressAction as CFString) == .success,
              find(root, "task-monitor-" + taskID) != nil else { print("FAIL return to Monitor"); return }
        print("PASS Inspect and Monitor return")
        do {
            guard try sessionIDs() == before else { print("UNAVAILABLE: Session population changed during navigation"); return }
            print("PASS configured Session identities unchanged; no Session action issued")
            succeeded = true
        } catch { print("FAIL final Session read", error) }
    }
}
let deadline = Date(timeIntervalSinceNow: 55)
while !finished && Date() < deadline { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1)) }
exit(succeeded ? 0 : 1)
