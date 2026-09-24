// Single-use owned New conversation proof. Do not rerun completed identities.
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
        for key in ["AXWindows", "AXChildren"] {
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
    guard let e = elements(root).first(where: { text($0,"AXRole") == "AXButton" && (text($0,"AXTitle") == title || text($0,"AXDescription") == title) }) else {
        print("Buttons:", elements(root).filter { text($0,"AXRole") == "AXButton" }.map { text($0,"AXTitle") + text($0,"AXDescription") })
        throw Failure(message: "Missing button " + title)
    }
    try check(AXUIElementPerformAction(e, kAXPressAction as CFString) == .success, "Press " + title)
    settle()
}

let proofRoot = "/tmp/loo291-iteration9"
let controlHome = "/Users/jack/.lf-dev/installed/local-be852452823d43d7b7fde663651a7590"
var environment = ProcessInfo.processInfo.environment
for key in ["LF_RUN_ID", "LF_RUN_DIR", "LF_PARENT_RUN_ID", "LF_PROCESS_ID", "LF_WORK_ADVANCE_CLAIM", "LOOPFLOW_DIRECTIVE_FILE", "LOOPFLOW_UI_TEST_MODE", "LOOPFLOW_UI_SNAPSHOT_PATH"] { environment.removeValue(forKey: key) }
environment["LF_HOME"] = controlHome
environment["LF_CONTROL_HOME"] = controlHome
environment["LF_CONTROL_DB_PATH"] = controlHome + "/loopflow.db"
environment["LF_CONTROL_BIN"] = "/Users/jack/.local/bin/lf"
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
    return try JSONSerialization.jsonObject(with: run("/Users/jack/.local/bin/lf", ["session", "list", "--json"])) as! [[String: Any]]
}
func json(_ path: String) throws -> [String: Any] {
    try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath:path))) as! [String: Any]
}
func save(_ value: Any, _ name: String) throws {
    try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted,.sortedKeys]).write(to: URL(fileURLWithPath: proofRoot + "/" + name))
}
let before = try sessions()
let prior = Set(before.compactMap { $0["id"] as? String })
let configuration = NSWorkspace.OpenConfiguration()
configuration.createsNewApplicationInstance = true
configuration.activates = true
configuration.arguments = ["--repo", "/Users/jack/src/loopflow.main-view-task"]
configuration.environment = environment
var finished = false
var exitStatus: Int32 = 0
let launchedAt = ProcessInfo.processInfo.systemUptime
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-iteration8/Loopflow Proof.app"), configuration: configuration) { running, error in
    guard let running else { print("FAIL launch",error?.localizedDescription ?? "unknown"); exitStatus=1; finished=true; return }
    print("Owned app PID", running.processIdentifier)
    DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
        defer { print("Owned app termination requested", running.terminate()); finished=true }
        let root = AXUIElementCreateApplication(running.processIdentifier)
        do {
            print("AX trust", AXIsProcessTrusted(), "active", running.isActive)
            var windows: CFTypeRef?
            print("AXWindows", AXUIElementCopyAttributeValue(root,kAXWindowsAttribute as CFString,&windows).rawValue,
                  "count", (windows as? [AXUIElement])?.count ?? -1)
            try check(AXIsProcessTrusted(), "Exact runner trusted")
            print("Activation requested", running.activate(options: [.activateAllWindows]))
            for window in (windows as? [AXUIElement] ?? []) {
                print("Raise owned window", AXUIElementPerformAction(window, kAXRaiseAction as CFString).rawValue)
            }
            settle(1)
            print("Active after request", running.isActive)
            let taskId = "workspace-task-ee671927-255f-41e6-8429-b830d59cc1de"
            let ready = ProcessInfo.processInfo.systemUptime + 20
            while !elements(root).contains(where: { text($0,"AXIdentifier") == taskId }) && ProcessInfo.processInfo.systemUptime < ready { settle(0.1) }
            if !elements(root).contains(where: { text($0,"AXIdentifier") == taskId }) {
                for e in elements(root) { print("BOUNDARY",text(e,"AXRole"),text(e,"AXIdentifier"),text(e,"AXTitle"),String(text(e,"AXValue").prefix(500))) }
            }
            let target = try find(root,id:taskId)
            print("TIMING launch_to_task_ax_ms",(ProcessInfo.processInfo.systemUptime-launchedAt)*1000)
            try check(AXUIElementPerformAction(target,kAXPressAction as CFString) == .success,"Select exact Task")
            settle()
            try check(running.isActive,"Owned app active before launch")

            try press(root,title:"All work")
            try press(root,title:"New terminal")
            settle(1)
            func send(_ input:String,enter:Bool=false) throws {
                try check(running.isActive,"Owned app active for input")
                let event=CGEvent(keyboardEventSource:nil,virtualKey:0,keyDown:true)!
                let chars=Array(input.utf16)
                chars.withUnsafeBufferPointer { event.keyboardSetUnicodeString(stringLength:chars.count,unicodeString:$0.baseAddress) }
                event.postToPid(running.processIdentifier); settle(0.2)
                if enter {
                    CGEvent(keyboardEventSource:nil,virtualKey:36,keyDown:true)!.postToPid(running.processIdentifier)
                    CGEvent(keyboardEventSource:nil,virtualKey:36,keyDown:false)!.postToPid(running.processIdentifier)
                    settle(0.2)
                }
            }

            try send("for i in {1..200}; do printf 'LOO291_HISTORY_%03d\\n' $i; done; printf 'LOO291_BOTTOM\\n'",enter:true)
            settle(1)
            for e in elements(root) { print("NODE",text(e,"AXRole"),text(e,"AXIdentifier"),text(e,"AXTitle"),text(e,"AXDescription"),String(text(e,"AXValue").prefix(200)),value(e,"AXPosition") as Any,value(e,"AXSize") as Any) }
            let ownWindows=(CGWindowListCopyWindowInfo([.optionOnScreenOnly,.excludeDesktopElements], kCGNullWindowID) as? [[String:Any]] ?? []).filter{ ($0[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == running.processIdentifier }
            try save(ownWindows,"windows.json")
            guard let window=ownWindows.first,let number=window[kCGWindowNumber as String] as? NSNumber,let bounds=window[kCGWindowBounds as String] as? [String:CGFloat] else { throw Failure(message:"Missing owned window") }
            _ = try run("/usr/sbin/screencapture",["-x","-l",number.stringValue,proofRoot+"/bottom.png"])
            let point=CGPoint(x:bounds["X"]!+bounds["Width"]!*0.7,y:bounds["Y"]!+bounds["Height"]!*0.7)
            for _ in 0..<8 {
                let wheel=CGEvent(scrollWheelEvent2Source:nil,units:.line,wheelCount:1,wheel1:12,wheel2:0,wheel3:0)!
                wheel.location=point
                wheel.postToPid(running.processIdentifier)
                settle(0.1)
            }
            settle(1)
            _ = try run("/usr/sbin/screencapture",["-x","-l",number.stringValue,proofRoot+"/scrolled.png"])
        } catch { print("FAIL",error); exitStatus=1 }
    }
}
while !finished { RunLoop.current.run(until:Date(timeIntervalSinceNow:0.1)) }
exit(exitStatus)
