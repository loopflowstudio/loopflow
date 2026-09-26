// Read-only, rerunnable configured observation. Never opens, moves or resolves a Session.
import AppKit
import ApplicationServices

func value(_ element: AXUIElement, _ key: String) -> CFTypeRef? {
    var result: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, key as CFString, &result) == .success else { return nil }
    return result
}
func elements(_ root: AXUIElement) -> [AXUIElement] {
    var seen: [AXUIElement] = []
    func walk(_ element: AXUIElement, _ depth: Int) -> [AXUIElement] {
        guard depth < 24, !seen.contains(where: { CFEqual($0, element) }) else { return [] }
        seen.append(element)
        guard value(element, "AXRole") as? String != "AXMenuBar" else { return [] }
        return [element] + ["AXWindows", "AXChildren"].flatMap { key in
            (value(element, key) as? [AXUIElement] ?? []).flatMap { walk($0, depth + 1) }
        }
    }
    return walk(root, 0)
}

struct ProofFailure: Error { let message: String }
func check(_ condition: Bool, _ message: String) throws {
    guard condition else { throw ProofFailure(message: message) }
    print("PASS", message)
}
func text(_ element: AXUIElement, _ key: String) -> String { value(element, key) as? String ?? "" }
func find(_ root: AXUIElement, _ id: String) throws -> AXUIElement {
    let deadline = ProcessInfo.processInfo.systemUptime + 5
    repeat {
        if let element = elements(root).first(where: { text($0, "AXIdentifier") == id }) { return element }
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
    } while ProcessInfo.processInfo.systemUptime < deadline
    throw ProofFailure(message: "Missing control: " + id)
}
func inspectPlanning(_ root: AXUIElement, records: [[String: Any]]) throws {
    let data = try Data(contentsOf: URL(fileURLWithPath: "scratch/configured-ui-evidence/iteration16-review/expected-planning.json"))
    let planning = try JSONSerialization.jsonObject(with: data) as! [String: Any]
    let task = planning["task"] as! [String: Any]
    let project = planning["project"] as! [String: Any]
    let taskID = task["id"] as! String
    let target = try find(root, "workspace-task-" + taskID)
    try check(AXUIElementPerformAction(target, kAXPressAction as CFString) == .success, "Select exact LOO-291 planning row")
    let started = ProcessInfo.processInfo.systemUptime
    let directive = try find(root, "workspace-task-directive")
    try check(text(directive, "AXValue") == task["description"] as! String, "Visible directive equals configured planning text")
    print("selection_to_directive_ax_ms", (ProcessInfo.processInfo.systemUptime - started) * 1000)
    let current = elements(root)
    try check(current.contains { text($0, "AXValue") == project["definition"] as! String }, "Visible Project definition equals configured planning text")
    for kr in project["krs"] as! [[String: Any]] {
        let label = ((kr["holds"] as! Bool) ? "Holds: " : "Not established: ") + (kr["text"] as! String)
        try check(current.contains { text($0, "AXDescription") == label || text($0, "AXTitle") == label || text($0, "AXValue") == label }, "Visible KR text and verdict equal configured planning")
    }
    let ownedWork = records.filter { ($0["work"] as? [String: String])?["id"] == "task_2aa71a7e36fe416d8a721e2b2f7c54e7" }
    try check(ownedWork.count == 1, "One exact Session association for selected Task")
    let session = ownedWork[0]
    _ = try find(root, "workspace-open-session-" + (session["id"] as! String))
    let toggle = try find(root, "workspace-toggle-list")
    try check(AXUIElementPerformAction(toggle, kAXPressAction as CFString) == .success, "Show work list alongside selected Task")
    let row = try find(root, "session-row-" + (session["id"] as! String))
    try check(text(row, "AXHelp").contains(session["work_path"] as! String), "Session row help contains shared Work display path")
    _ = try find(root, "workspace-open-session-" + (session["id"] as! String))
    print("PASS Exact Session access is present; no Session button was pressed")
    try check(current.contains { text($0, "AXValue").hasPrefix("Planning read: ") }, "Planning read timestamp is visible")
    let projectButton = try find(root, "workspace-project-" + (project["id"] as! String))
    try check(AXUIElementPerformAction(projectButton, kAXPressAction as CFString) == .success, "Select Project without opening a provider")
    _ = try find(root, "workspace-no-sessions")
    print("PASS Explicit no-Session state for selected Project")
}

setbuf(stdout, nil)
print("Observed at", ISO8601DateFormatter().string(from: Date()))
print("Runner", CommandLine.arguments[0], "AX trusted", AXIsProcessTrusted())
let console = CGSessionCopyCurrentDictionary() as? [String: Any]
if (console?["CGSSessionScreenIsLocked"] as? NSNumber)?.boolValue == true {
    print("UNAVAILABLE: macOS desktop is locked. Unlock this Mac before running configured UI proof. No app launched.")
    exit(2)
}
guard AXIsProcessTrusted() else {
    print("UNAVAILABLE: this exact runner lacks Accessibility trust. No app launched.")
    exit(2)
}
let home = "/Users/jack/.lf-dev/installed/local-be852452823d43d7b7fde663651a7590"
let cli = "/Users/jack/src/loopflow.main-view-task/target/debug/lf"
let repo = "/Users/jack/src/loopflow.main-view-task"
var environment = ProcessInfo.processInfo.environment
for key in ["LF_RUN_ID", "LF_RUN_DIR", "LF_PARENT_RUN_ID", "LF_PROCESS_ID", "LF_WORK_ADVANCE_CLAIM", "LF_WAVE_ID", "LF_DIRECTIVE_FILE", "LOOPFLOW_DIRECTIVE_FILE", "LOOPFLOW_UI_TEST_MODE", "LOOPFLOW_UI_SNAPSHOT_PATH"] { environment.removeValue(forKey: key) }
environment["LF_HOME"] = home
environment["LF_CONTROL_HOME"] = home
environment["LF_DB_PATH"] = home + "/loopflow.db"
environment["LF_CONTROL_DB_PATH"] = home + "/loopflow.db"
environment["LF_BIN"] = cli
environment["LF_CONTROL_BIN"] = cli
func sessions() throws -> [[String: Any]] {
    let process = Process(), output = Pipe()
    process.executableURL = URL(fileURLWithPath: cli)
    process.arguments = ["session", "list", "--json"]
    process.currentDirectoryURL = URL(fileURLWithPath: repo)
    process.environment = environment
    process.standardOutput = output
    process.standardError = FileHandle.standardError
    try process.run()
    let data = output.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { throw NSError(domain: "ConfiguredSessionRead", code: Int(process.terminationStatus)) }
    return try JSONSerialization.jsonObject(with: data) as! [[String: Any]]
}
let before = try sessions()
let expected = "\(before.count) Sessions"
let configuration = NSWorkspace.OpenConfiguration()
configuration.createsNewApplicationInstance = true
configuration.activates = true
configuration.arguments = ["--repo", repo]
configuration.environment = environment
var finished = false
var succeeded = false
let started = ProcessInfo.processInfo.systemUptime
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-iteration16/Loopflow Proof.app"), configuration: configuration) { app, error in
    guard let app else { print("FAIL launch", error?.localizedDescription ?? "unknown"); finished = true; return }
    print("Owned app PID", app.processIdentifier)
    DispatchQueue.main.async {
        defer { print("Owned app termination requested", app.terminate()); finished = true }
        let root = AXUIElementCreateApplication(app.processIdentifier)
        let deadline = ProcessInfo.processInfo.systemUptime + 15
        var windowRoles: [String] = []
        repeat {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
            windowRoles = (value(root, "AXWindows") as? [AXUIElement] ?? []).map { value($0, "AXRole") as? String ?? "unavailable" }
            // An AXWindows entry that is the application itself is not window proof.
            if windowRoles.contains("AXWindow"), elements(root).contains(where: { value($0, "AXIdentifier") as? String == "podium-sessions" && value($0, "AXValue") as? String == expected }) {
                succeeded = true
                break
            }
        } while ProcessInfo.processInfo.systemUptime < deadline
        print("AX window roles", windowRoles, "active", app.isActive)
        print("Count observed", succeeded, "expected", expected, "elapsed_ms", (ProcessInfo.processInfo.systemUptime - started) * 1000)
        do {
            guard succeeded else { throw ProofFailure(message: "Count unavailable") }
            try inspectPlanning(root, records: before)
            let after = try sessions()
            let beforeIDs = Set(before.compactMap { $0["id"] as? String })
            let afterIDs = Set(after.compactMap { $0["id"] as? String })
            guard beforeIDs == afterIDs else { succeeded = false; print("UNAVAILABLE: Session population changed during observation"); return }
        } catch { succeeded = false; print("FAIL configured planning observation", error) }
        print(succeeded ? "PASS: configured planning, exact Session access and shared Work path observed; no Session interaction" : "FAIL: configured planning proof incomplete; preceding observations retain their scope")
    }
}
while !finished { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1)) }
exit(succeeded ? 0 : 1)
