// Owned configured editor trial: edit local draft, Cancel, reopen. No Save or provider actions.
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
        for key in ["AXWindows", "AXChildren", "AXSheets"] {
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
    if title == "Show work list", elements(root).contains(where: { text($0,"AXDescription") == "Hide work list" }) { return }
    guard let e = elements(root).first(where: { text($0,"AXRole") == "AXButton" && (text($0,"AXTitle") == title || text($0,"AXDescription") == title) }) else {
        print("Buttons:", elements(root).filter { text($0,"AXRole") == "AXButton" }.map { text($0,"AXTitle") + text($0,"AXDescription") })
        throw Failure(message: "Missing button " + title)
    }
    try check(AXUIElementPerformAction(e, kAXPressAction as CFString) == .success, "Press " + title)
    settle()
}

func focusedPID() -> pid_t {
    var focused: CFTypeRef?
    let status=AXUIElementCopyAttributeValue(AXUIElementCreateSystemWide(),kAXFocusedApplicationAttribute as CFString,&focused)
    guard status == .success,let focused else { print("Focused application status",status.rawValue); return -1 }
    var pid: pid_t = -1
    AXUIElementGetPid(focused as! AXUIElement,&pid)
    return pid
}
setbuf(stdout, nil)
let proofRoot = "/tmp/loo291-iteration13-editor-sheet-result"
let controlHome = "/Users/jack/.lf-dev/installed/local-be852452823d43d7b7fde663651a7590"
var environment = ProcessInfo.processInfo.environment
for key in ["LF_RUN_ID", "LF_RUN_DIR", "LF_PARENT_RUN_ID", "LF_PROCESS_ID", "LF_WORK_ADVANCE_CLAIM", "LOOPFLOW_DIRECTIVE_FILE", "LOOPFLOW_UI_TEST_MODE", "LOOPFLOW_UI_SNAPSHOT_PATH"] { environment.removeValue(forKey: key) }
environment["LF_HOME"] = controlHome
environment["LF_DB_PATH"] = controlHome + "/loopflow.db"
environment["LF_BIN"] = "/Users/jack/src/loopflow.main-view-task/target/debug/lf"
environment["LF_CONTROL_HOME"] = controlHome
environment["LF_CONTROL_DB_PATH"] = controlHome + "/loopflow.db"
environment["LF_CONTROL_BIN"] = "/Users/jack/src/loopflow.main-view-task/target/debug/lf"
environment.removeValue(forKey: "LF_WAVE_ID")
func run(_ executable: String, _ args: [String]) throws -> Data {
    let process = Process(), output = Pipe()
    process.executableURL = URL(fileURLWithPath: executable)
    process.arguments = args
    process.environment = environment
    process.currentDirectoryURL = URL(fileURLWithPath: "/Users/jack/src/loopflow.main-view-task")
    process.standardOutput = output
    process.standardError = FileHandle.nullDevice
    try process.run()
    let result = output.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { throw Failure(message: "Command failed: " + args.joined(separator: " ")) }
    return result
}
func sessions() throws -> [[String: Any]] {
    return try JSONSerialization.jsonObject(with: run("/Users/jack/src/loopflow.main-view-task/target/debug/lf", ["session", "list", "--json"])) as! [[String: Any]]
}
func json(_ path: String) throws -> [String: Any] {
    try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath:path))) as! [String: Any]
}
func save(_ value: Any, _ name: String) throws {
    try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted,.sortedKeys]).write(to: URL(fileURLWithPath: proofRoot + "/" + name))
}
try FileManager.default.createDirectory(atPath: proofRoot, withIntermediateDirectories: false)

let cli = "/Users/jack/src/loopflow.main-view-task/target/debug/lf"
func directive() throws -> String {
    let roadmap = try JSONSerialization.jsonObject(with: run(cli,["roadmap","--all","--json"])) as! [String:Any]
    for wave in roadmap["waves"] as? [[String:Any]] ?? [] {
        let projects = wave["projects"] as? [String:Any] ?? [:]
        for project in projects["items"] as? [[String:Any]] ?? [] {
            for row in project["tasks"] as? [[String:Any]] ?? [] {
                let task = row["task"] as? [String:Any] ?? [:]
                if task["identifier"] as? String == "LOO-291" { return task["description"] as! String }
            }
        }
    }
    throw Failure(message:"Task absent from configured roadmap")
}
let original = try directive()
try save(try sessions(),"sessions-before.json")
let configuration = NSWorkspace.OpenConfiguration()
configuration.createsNewApplicationInstance = true
configuration.activates = false
configuration.arguments = ["--repo", "/Users/jack/src/loopflow.main-view-task"]
configuration.environment = environment
var finished = false
var exitStatus: Int32 = 0
let launchedAt = ProcessInfo.processInfo.systemUptime
print("observed_at",ISO8601DateFormatter().string(from:Date()))
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-iteration12-editor/Loopflow Editor.app"), configuration: configuration) { running, error in
    guard let running else { print("FAIL launch",error?.localizedDescription ?? "unknown"); exitStatus=1; finished=true; return }
    print("Owned app PID",running.processIdentifier)
    DispatchQueue.main.asyncAfter(deadline:.now()+1) {
        defer { print("Owned app termination requested",running.terminate()); finished=true }
        let root=AXUIElementCreateApplication(running.processIdentifier)
        do {
            print("AX trust",AXIsProcessTrusted(),"active",running.isActive)
            var windows: CFTypeRef?
            print("AXWindows",AXUIElementCopyAttributeValue(root,kAXWindowsAttribute as CFString,&windows).rawValue,"count",(windows as? [AXUIElement])?.count ?? -1)
            try check(AXIsProcessTrusted(),"Exact runner trusted")
            // These actions address exact AX elements in this owned app. No global input is sent.
            print("System focused PID (diagnostic only)",focusedPID(),"owned",running.processIdentifier)
            try check((windows as? [AXUIElement])?.count == 1,"One owned accessible window")
            let taskId="workspace-task-ee671927-255f-41e6-8429-b830d59cc1de"
            let deadline=ProcessInfo.processInfo.systemUptime+20
            while !elements(root).contains(where:{text($0,"AXIdentifier")==taskId}) && ProcessInfo.processInfo.systemUptime<deadline { settle(0.1) }
            if !elements(root).contains(where:{text($0,"AXIdentifier")==taskId}) {
                print("Accessible controls", elements(root).map { [text($0,"AXRole"),text($0,"AXIdentifier"),text($0,"AXTitle"),text($0,"AXDescription")].joined(separator:" | ") })
            }
            let target=try find(root,id:taskId)
            print("TIMING launch_to_task_ax_ms",(ProcessInfo.processInfo.systemUptime-launchedAt)*1000)
            try check(AXUIElementPerformAction(target,kAXPressAction as CFString) == .success,"Select exact Task")
            settle()
            let start=ProcessInfo.processInfo.systemUptime
            try press(root,title:"Edit directive")
            let editorDeadline=ProcessInfo.processInfo.systemUptime+5
            while !elements(root).contains(where:{text($0,"AXIdentifier")=="task-directive-draft"}) && ProcessInfo.processInfo.systemUptime<editorDeadline { settle(0.1) }
            print("Editor tree", elements(root).map { [text($0,"AXRole"),text($0,"AXIdentifier"),text($0,"AXTitle"),text($0,"AXDescription")].joined(separator:" | ") })
            let editor=try find(root,id:"task-directive-draft")
            print("TIMING edit_to_editor_ax_ms",(ProcessInfo.processInfo.systemUptime-start)*1000)
            print("Editor role",text(editor,"AXRole"))
            try check(text(editor,"AXValue")==original,"Editor loads exact authoritative directive")
            let draft="Unsubmitted review draft.\n--Literal text stays local."
            try check(AXUIElementSetAttributeValue(editor,kAXValueAttribute as CFString,draft as CFTypeRef) == .success,"Set owned local editor draft")
            settle()
            try check(text(editor,"AXValue")==draft,"Exact draft visible before Cancel")
            try press(root,title:"Cancel")
            try check(!elements(root).contains(where:{text($0,"AXIdentifier")=="task-directive-draft"}),"Cancel dismisses editor")
            try press(root,title:"Edit directive")
            let reopened=try find(root,id:"task-directive-draft")
            try check(text(reopened,"AXValue")==original,"Reopening discards cancelled draft")
            try press(root,title:"Cancel")
            try check(try directive()==original,"Shared directive unchanged after Cancel")
        } catch { print("FAIL",error); exitStatus=1 }
    }
}
while !finished { RunLoop.current.run(until:Date(timeIntervalSinceNow:0.1)) }
try save(try sessions(),"sessions-after.json")
exit(exitStatus)
