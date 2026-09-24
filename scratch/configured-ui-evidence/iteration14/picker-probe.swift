// Single-use configured nested-workspace trial. Targeted AX actions only; no keyboard events.
// Single-use review probe; writes only a disposable shell receipt under /tmp.
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
let proofRoot = "/tmp/loo291-iteration14-picker-result"
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
let before = try sessions()
try save(before, "sessions-before.json")
let prior = Set(before.compactMap { $0["id"] as? String })

let config=NSWorkspace.OpenConfiguration()
config.createsNewApplicationInstance=true
config.activates=false
config.arguments=["--repo","/Users/jack/src/loopflow.main-view-task"]
config.environment=environment
var finished=false
NSWorkspace.shared.openApplication(at:URL(fileURLWithPath:"/tmp/loo291-iteration14/Loopflow Nested.app"),configuration:config) { running,error in
    guard let running else { finished=true; return }
    print("Owned app PID",running.processIdentifier)
    DispatchQueue.main.asyncAfter(deadline:.now()+1) {
        defer { print("Terminated",running.terminate()); finished=true }
        let root=AXUIElementCreateApplication(running.processIdentifier)
        do {
            print("Trust",AXIsProcessTrusted())
            let deadline=Date(timeIntervalSinceNow:20)
            while !elements(root).contains(where:{text($0,"AXRole")=="AXMenuButton" && text($0,"AXTitle")=="2"}) && Date()<deadline { settle(0.2) }
            guard let picker=elements(root).first(where:{text($0,"AXRole")=="AXMenuButton" && text($0,"AXTitle")=="2"}) else { throw Failure(message:"Missing picker") }
            var actions:CFArray?
            print("Actions status",AXUIElementCopyActionNames(picker,&actions).rawValue,"actions",actions ?? [] as CFArray)
            print("Press status",AXUIElementPerformAction(picker,kAXPressAction as CFString).rawValue)
            settle(1)
            for e in elements(root) where text(e,"AXRole")=="AXMenuItem" { print("MENU",text(e,"AXIdentifier"),text(e,"AXTitle")) }
            try save(elements(root).map{["role":text($0,"AXRole"),"id":text($0,"AXIdentifier"),"label":text($0,"AXDescription"),"title":text($0,"AXTitle")]},"controls.json")
        } catch { print("FAIL",error) }
    }
}
while !finished { settle(0.1) }
