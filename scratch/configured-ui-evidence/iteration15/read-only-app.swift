import AppKit
import ApplicationServices

func value(_ element: AXUIElement, _ key: String) -> CFTypeRef? {
    var result: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, key as CFString, &result) == .success else { return nil }
    return result
}
func elements(_ root: AXUIElement) -> [AXUIElement] {
    var seen = Set<CFHashCode>()
    func walk(_ element: AXUIElement, _ depth: Int) -> [AXUIElement] {
        guard depth < 24, seen.insert(CFHash(element)).inserted else { return [] }
        return [element] + ["AXWindows", "AXChildren"].flatMap { key in
            (value(element, key) as? [AXUIElement] ?? []).flatMap { walk($0, depth + 1) }
        }
    }
    return walk(root, 0)
}
let home = "/Users/jack/.lf-dev/installed/local-be852452823d43d7b7fde663651a7590"
let cli = "/Users/jack/src/loopflow.main-view-task/target/debug/lf"
var environment = ProcessInfo.processInfo.environment
for key in ["LF_RUN_ID", "LF_RUN_DIR", "LF_PARENT_RUN_ID", "LF_PROCESS_ID", "LF_WORK_ADVANCE_CLAIM", "LF_WAVE_ID", "LF_DIRECTIVE_FILE", "LOOPFLOW_DIRECTIVE_FILE", "LOOPFLOW_UI_TEST_MODE", "LOOPFLOW_UI_SNAPSHOT_PATH"] { environment.removeValue(forKey: key) }
environment["LF_HOME"] = home
environment["LF_CONTROL_HOME"] = home
environment["LF_DB_PATH"] = home + "/loopflow.db"
environment["LF_CONTROL_DB_PATH"] = home + "/loopflow.db"
environment["LF_BIN"] = cli
environment["LF_CONTROL_BIN"] = cli
let configuration = NSWorkspace.OpenConfiguration()
configuration.createsNewApplicationInstance = true
configuration.activates = false
configuration.arguments = ["--repo", "/Users/jack/src/loopflow.main-view-task"]
configuration.environment = environment
var finished = false
var succeeded = false
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-iteration15/Loopflow Shared Sessions.app"), configuration: configuration) { app, error in
    guard let app else { print("FAIL launch", error?.localizedDescription ?? "unknown"); finished = true; return }
    defer { print("Owned app termination requested", app.terminate()); finished = true }
    print("Owned app PID", app.processIdentifier, "AX trust", AXIsProcessTrusted())
    let root = AXUIElementCreateApplication(app.processIdentifier)
    let deadline = Date(timeIntervalSinceNow: 12)
    var summary = ""
    repeat {
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
        if let control = elements(root).first(where: { value($0, "AXIdentifier") as? String == "podium-sessions" }) {
            summary = value(control, "AXValue") as? String ?? ""
        }
        if summary == "4 Sessions" { succeeded = true; break }
    } while Date() < deadline
    print("Owned AX windows", (value(root, "AXWindows") as? [AXUIElement])?.count ?? 0, "active", app.isActive)
    print(succeeded ? "PASS" : "FAIL", "configured shared Session count", summary)
}
while !finished { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1)) }
exit(succeeded ? 0 : 1)
