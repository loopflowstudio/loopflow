import AppKit
let appURL = URL(fileURLWithPath: "/tmp/loo291-post1281/Loopflow Review.app")
let configuration = NSWorkspace.OpenConfiguration()
configuration.createsNewApplicationInstance = true
configuration.activates = true
configuration.arguments = ["--repo", "/Users/jack/src/loopflow.main-view-task"]
var environment = ProcessInfo.processInfo.environment
for key in ["LF_RUN_ID", "LF_RUN_DIR", "LF_PARENT_RUN_ID", "LF_PROCESS_ID", "LF_TRACE_ID", "LF_WORK_ADVANCE_CLAIM", "LF_HUMAN_SESSION", "LOOPFLOW_DIRECTIVE_FILE", "LOOPFLOW_UI_TEST_MODE", "LOOPFLOW_UI_SNAPSHOT_PATH", "LF_WAVE_ID", "LF_TERMINAL_ID"] {
    environment.removeValue(forKey: key)
}
let home = "/Users/jack/.lf-dev/installed/local-198f5df536e545b7b63b3d15abeacdd2"
environment["LF_HOME"] = home
environment["LF_CONTROL_HOME"] = home
environment["LF_CONTROL_DB_PATH"] = home + "/loopflow.db"
environment["LF_CONTROL_BIN"] = appURL.appendingPathComponent("Contents/MacOS/lf").path
environment["LF_BIN"] = environment["LF_CONTROL_BIN"]
configuration.environment = environment
var finished = false
NSWorkspace.shared.openApplication(at: appURL, configuration: configuration) { app, error in
    if let app {
        print("Opened Loopflow Review PID \(app.processIdentifier)")
        print("Active: \(app.isActive)")
    } else {
        print("Launch failed: \(error?.localizedDescription ?? "unknown")")
    }
    finished = true
}
while !finished { RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1)) }
