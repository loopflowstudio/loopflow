import AppKit
let configuration = NSWorkspace.OpenConfiguration()
configuration.createsNewApplicationInstance = true
configuration.activates = true
configuration.arguments = ["--repo", "/Users/jack/src/loopflow.main-view-task"]
var environment = ProcessInfo.processInfo.environment
for key in ["LF_RUN_ID", "LF_RUN_DIR", "LF_PARENT_RUN_ID", "LF_PROCESS_ID", "LF_WORK_ADVANCE_CLAIM", "LOOPFLOW_DIRECTIVE_FILE", "LOOPFLOW_UI_TEST_MODE", "LOOPFLOW_UI_SNAPSHOT_PATH", "LF_WAVE_ID"] {
    environment.removeValue(forKey: key)
}
let home = "/Users/jack/.lf-dev/installed/local-be852452823d43d7b7fde663651a7590"
let cli = "/Users/jack/src/loopflow.main-view-task/target/debug/lf"
environment["LF_HOME"] = home
environment["LF_CONTROL_HOME"] = home
environment["LF_DB_PATH"] = home + "/loopflow.db"
environment["LF_CONTROL_DB_PATH"] = home + "/loopflow.db"
environment["LF_BIN"] = cli
environment["LF_CONTROL_BIN"] = cli
configuration.environment = environment
var finished = false
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-iteration12-editor/Loopflow Editor.app"), configuration: configuration) { app, error in
    if let app { print("Review app PID", app.processIdentifier) }
    else { print("Launch failed", error?.localizedDescription ?? "unknown") }
    finished = true
}
while !finished { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1)) }
