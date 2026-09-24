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
            let after = try sessions()
            let beforeIDs = Set(before.compactMap { $0["id"] as? String })
            let afterIDs = Set(after.compactMap { $0["id"] as? String })
            guard beforeIDs == afterIDs else { succeeded = false; print("UNAVAILABLE: Session population changed during observation"); return }
        } catch { succeeded = false; print("FAIL final Session read", error) }
        print(succeeded ? "PASS: configured scoped count observed; no Session interaction" : "FAIL: configured scoped count not established")
    }
}
while !finished { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1)) }
exit(succeeded ? 0 : 1)
