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
        guard depth < 24, !seen.contains(where: { CFEqual($0, element) }) else { return [] }; seen.append(element)
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
configuration.activates = true
configuration.arguments = ["--repo", "/Users/jack/src/loopflow.main-view-task"]
configuration.environment = environment
var finished = false
var succeeded = false
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-iteration16/Loopflow Proof.app"), configuration: configuration) { app, error in
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
    for (index, window) in (value(root, "AXWindows") as? [AXUIElement] ?? []).enumerated() {
        var pid: pid_t = 0
        AXUIElementGetPid(window, &pid)
        var names: CFArray?
        let status = AXUIElementCopyAttributeNames(window, &names)
        print("Window", index, "PID", pid, "equals root", CFEqual(window, root), "attributes status", status.rawValue, "names", names as Any)
        for key in ["AXRole", "AXTitle", "AXChildren", "AXVisible", "AXMinimized", "AXPosition", "AXSize"] {
            var raw: CFTypeRef?
            let status = AXUIElementCopyAttributeValue(window, key as CFString, &raw)
            print("window attribute", key, "status", status.rawValue, "value", raw as Any)
        }
        print("Window descendants", elements(window).count)
    }
    let rows = elements(root).map { element in
        ["AXRole", "AXIdentifier", "AXTitle", "AXDescription", "AXValue", "AXEnabled"].reduce(into: [String: String]()) { row, key in
            if let v = value(element, key) { row[key] = String(describing: v).prefix(500).description }
        }
    }
    let data = try! JSONSerialization.data(withJSONObject: rows, options: [.prettyPrinted, .sortedKeys])
    try! data.write(to: URL(fileURLWithPath: "/Users/jack/src/loopflow.main-view-task/scratch/configured-ui-evidence/iteration16/controls.json"))
    print("AX nodes", rows.count)
    print(succeeded ? "PASS" : "FAIL", "configured shared Session count", summary)
}
while !finished { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1)) }
exit(succeeded ? 0 : 1)
