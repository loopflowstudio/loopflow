// Read-only configured navigation; never opens, transfers or resolves a Session.
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
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-review20/Loopflow Proof.app"), configuration: configuration) { app, error in
    guard let app else { print("FAIL launch", error?.localizedDescription ?? "unknown"); finished = true; return }
    print("Owned app PID", app.processIdentifier, "Home", home)
    DispatchQueue.main.async {
        defer { print("Owned app termination requested", app.terminate()); finished = true }
        let root = AXUIElementCreateApplication(app.processIdentifier)
        AXUIElementSetMessagingTimeout(root, 2)
        guard let search = find(root, "workspace-search") ?? elements(root).first(where: { value($0, "AXRole") as? String == "AXTextField" }) else {
            print("FAIL no outline search; active", app.isActive)
            print("Window roles", (value(root, "AXWindows") as? [AXUIElement] ?? []).map { value($0, "AXRole") as? String ?? "unknown" })
            print("Available identifiers", elements(root).compactMap { value($0, "AXIdentifier") as? String })
            return
        }
        guard AXUIElementSetAttributeValue(search, "AXValue" as CFString, "LOO-291" as CFString) == .success else { print("FAIL search input"); return }
        guard let task = find(root, "workspace-task-" + taskID),
              AXUIElementPerformAction(task, kAXPressAction as CFString) == .success,
              let refresh = find(root, "task-monitor-" + taskID).flatMap({ _ in elements(root).first { value($0, "AXRole") as? String == "AXButton" && (value($0, "AXTitle") as? String == "Refresh" || value($0, "AXDescription") as? String == "Refresh") } }) else {
            print("FAIL exact Task selection did not open Monitor")
            print("Identifiers", elements(root).compactMap { value($0, "AXIdentifier") as? String })
            return
        }
        print("PASS configured Task selected by exact planning ID", taskID, "and Monitor refresh available")
        do {
            let snapshot = try query(["runs", "--active", "--task", "LOO-291", "--json"]) as! [String: Any]
            let runs = snapshot["runs"] as! [[String: Any]]
            let gaps = snapshot["gaps"] as! [String]
            let expected = Set(runs.compactMap { $0["id"] as? String })
            let deadline = Date(timeIntervalSinceNow: 12)
            var matched = false
            repeat {
                let nodes = elements(root)
                let displayed = Set(nodes.compactMap { value($0, "AXIdentifier") as? String }.filter { $0.hasPrefix("monitor-run-") }.map { String($0.dropFirst(12)) })
                let empty = nodes.contains { value($0, "AXIdentifier") as? String == "monitor-empty-" + taskID || value($0, "AXValue") as? String == "No active Runs in this observation" }
                let observed = nodes.contains { (value($0, "AXValue") as? String)?.hasPrefix("Observed ") == true }
                matched = displayed == expected && observed && (!expected.isEmpty || !gaps.isEmpty || empty)
                if matched { break }
                RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
            } while Date() < deadline
            guard matched else {
                print("FAIL Monitor observation differs from CLI", expected, "gaps", gaps.count)
                print("Controls", elements(root).map { ["id": value($0, "AXIdentifier") as? String ?? "", "value": value($0, "AXValue") as? String ?? "", "title": value($0, "AXTitle") as? String ?? ""] }.filter { !$0.values.allSatisfy(\.isEmpty) })
                return
            }
            print("PASS Monitor exact Run IDs match sequential CLI observation", expected.sorted(), "gaps", gaps.count)
            guard AXUIElementPerformAction(refresh, kAXPressAction as CFString) == .success else { print("FAIL refresh"); return }
            print("PASS explicit Refresh accepted")
        } catch { print("FAIL active read", error); return }
        guard let inspect = elements(root).first(where: { value($0, "AXRole") as? String == "AXButton" && (value($0, "AXTitle") as? String == "Inspect" || value($0, "AXDescription") as? String == "Inspect") }),
              AXUIElementPerformAction(inspect, kAXPressAction as CFString) == .success,
              find(root, "workspace-task-directive") != nil else { print("FAIL Inspect did not expose directive"); return }
        print("PASS Inspect retains Task directive access")
        guard let presentation = find(root, "workspace-presentation") ?? elements(root).first(where: { value($0, "AXDescription") as? String == "Outline presentation" }),
              AXUIElementPerformAction(presentation, kAXPressAction as CFString) == .success else { print("FAIL presentation menu"); return }
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.3))
        let titles = elements(root).compactMap { value($0, "AXTitle") as? String }
        guard ["Compact", "Full hierarchy", "Sessions"].allSatisfy(titles.contains) else {
            print("FAIL missing presentation choices", titles.filter { !$0.isEmpty }.suffix(15)); return
        }
        print("PASS one menu exposes Compact, Full hierarchy and Sessions")
        if let full = elements(root).first(where: { value($0, "AXTitle") as? String == "Full hierarchy" }) {
            guard AXUIElementPerformAction(full, kAXPressAction as CFString) == .success,
                  find(root, "workspace-task-" + taskID) != nil,
                  find(root, "workspace-task-directive") != nil else { print("FAIL Full presentation continuity"); return }
            print("PASS Full presentation preserves selected Task and inspection")
        }
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
