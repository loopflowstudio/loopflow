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
let proofRoot = "/tmp/loo291-iteration14-nested-result"
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
let configuration = NSWorkspace.OpenConfiguration()
configuration.createsNewApplicationInstance = true
configuration.activates = false
configuration.arguments = ["--repo", "/Users/jack/src/loopflow.main-view-task"]
configuration.environment = environment
var finished = false
var exitStatus: Int32 = 0
print("observed_at", ISO8601DateFormatter().string(from: Date()))
NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: "/tmp/loo291-iteration12-editor/Loopflow Editor.app"), configuration: configuration) { running, error in
    guard let running else { print("FAIL launch", error?.localizedDescription ?? "unknown"); exitStatus=1; finished=true; return }
    print("Owned app PID", running.processIdentifier)
    DispatchQueue.main.asyncAfter(deadline: .now()+1) {
        defer { print("Owned app termination requested", running.terminate()); finished=true }
        let root = AXUIElementCreateApplication(running.processIdentifier)
        do {
            print("AX trust", AXIsProcessTrusted(), "active", running.isActive)
            try check(AXIsProcessTrusted(), "Exact runner trusted")
            try check((value(root,"AXWindows") as? [AXUIElement])?.count == 1, "One owned accessible window")
            func waitFor(_ predicate: () -> Bool, seconds: Double = 10) -> Bool {
                let deadline=Date(timeIntervalSinceNow:seconds)
                while !predicate() && Date()<deadline { settle(0.1) }
                return predicate()
            }
            func byID(_ id: String) throws -> AXUIElement {
                _ = waitFor { elements(root).contains { text($0,"AXIdentifier")==id } }
                return try find(root,id:id)
            }
            func click(_ element: AXUIElement, _ description: String) throws {
                try check(AXUIElementPerformAction(element,kAXPressAction as CFString) == .success,description)
                settle()
            }
            func panes() -> [AXUIElement] { elements(root).filter { text($0,"AXIdentifier")=="session-pane-empty" } }
            func shellReceipts() throws -> [String] {
                let output=String(decoding:try run("/bin/ps",["-axo","pid=,ppid=,lstart=,comm="]),as:UTF8.self)
                return output.split(separator:"\n").compactMap { line in
                    let fields=line.split(whereSeparator: {$0.isWhitespace})
                    guard fields.count>=8,Int32(fields[1])==running.processIdentifier,fields.last.map({["zsh","bash","fish"].contains(URL(fileURLWithPath:String($0)).lastPathComponent)}) == true else { return nil }
                    return String(line).trimmingCharacters(in:.whitespaces)
                }.sorted()
            }
            let taskID="workspace-task-ee671927-255f-41e6-8429-b830d59cc1de"
            _ = try byID(taskID)
            try press(root,title:"All work")
            try press(root,title:"New terminal")
            try check(waitFor { panes().count==1 },"One main checkout pane")
            try press(root,title:"Show work list")
            try click(try byID(taskID),"Select exact Task")
            try press(root,title:"New conversation · LOO-291")
            var owned: [String:Any]?
            let deadline=Date(timeIntervalSinceNow:60)
            while owned==nil && Date()<deadline {
                owned=try sessions().first { row in
                    guard let id=row["id"] as? String,!prior.contains(id),let work=row["work"] as? [String:Any] else { return false }
                    return work["id"] as? String=="task_2aa71a7e36fe416d8a721e2b2f7c54e7"
                }
                if owned==nil { settle(0.3) }
            }
            guard let record=owned,let id=record["id"] as? String else { throw Failure(message:"No new exact Task Session") }
            try save(record,"owned-session.json")
            print("Owned Session",id)
            try check(!(record["terminal_ids"] as? [String] ?? []).isEmpty,"Shared Session has native terminal attachment")
            let dir=controlHome+"/runs/"+String(id.dropFirst(4).prefix(2))+"/"+id
            func client() throws -> [String:Any] {
                let files=try FileManager.default.contentsOfDirectory(atPath:dir+"/provider-clients")
                let clients=try files.filter{$0.hasSuffix(".json")}.map{try json(dir+"/provider-clients/"+$0)}.filter{kill(($0["pid"] as? NSNumber)?.int32Value ?? 0,0)==0}
                guard clients.count==1 else { throw Failure(message:"Expected one live owned provider") }
                return clients[0]
            }
            let initialClient=try client()
            let providerPID=(initialClient["pid"] as! NSNumber).int32Value
            var ancestor=providerPID
            for _ in 0..<12 {
                if ancestor==running.processIdentifier || ancestor<=1 { break }
                ancestor=Int32(String(decoding:try run("/bin/ps",["-p",String(ancestor),"-o","ppid="]),as:UTF8.self).trimmingCharacters(in:.whitespacesAndNewlines)) ?? 0
            }
            try check(ancestor==running.processIdentifier,"Exact provider descends from owned app")
            try save(initialClient,"client-before.json")
            try press(root,title:"Show work list")
            let row=try byID("session-row-"+id)
            try click(row,"Select exact Session row into originating shell")
            try check(NSDictionary(dictionary:initialClient).isEqual(to:try client()),"Row selection retains provider PID and birth receipt")
            try check(!elements(root).contains{text($0,"AXIdentifier")=="session-move-here-"+id},"Local row requests no transfer")
            try press(root,title:"New terminal")
            try check(waitFor{panes().count==2},"Task retains provider shell and companion")
            try press(root,title:"Split worktrees right")
            guard let picker=elements(root).first(where:{["AXMenuButton","AXPopUpButton"].contains(text($0,"AXRole")) && (text($0,"AXDescription")=="Choose worktree" || text($0,"AXTitle")=="Choose worktree")}) else {
                print("PICKERS",elements(root).filter{["AXMenuButton","AXPopUpButton"].contains(text($0,"AXRole"))}.map{[text($0,"AXRole"),text($0,"AXDescription"),text($0,"AXTitle")]})
                throw Failure(message:"No empty outer worktree picker")
            }
            try check(AXUIElementPerformAction(picker,kAXShowMenuAction as CFString) == .success,"Open worktree picker")
            settle()
            guard let main=elements(root).first(where:{text($0,"AXRole")=="AXMenuItem" && text($0,"AXTitle")=="loopflow"}) else {
                print("MENU",elements(root).filter{text($0,"AXRole")=="AXMenuItem"}.map{text($0,"AXTitle")})
                throw Failure(message:"No retained main checkout option")
            }
            try click(main,"Restore retained main checkout beside Task")
            try press(root,title:"New terminal")
            try check(waitFor{panes().count==4},"Two checkout groups each retain two native panes")
            let initialShells=try shellReceipts()
            try save(initialShells,"shells-before.json")
            try check(initialShells.count==4,"Four direct child shells have process receipts")
            let paneLabels=panes().map{text($0,"AXDescription")}
            print("PANE_LABELS_BEFORE",paneLabels)
            try click(try byID("session-row-"+id),"Return from main checkout through exact Session row")
            try check(panes().count==4,"Row return preserves both nested layouts")
            guard let completion=elements(root).first(where:{text($0,"AXRole")=="AXButton" && text($0,"AXDescription")=="Complete session"}) else { throw Failure(message:"Missing local completion control") }
            var parent: AXUIElement? = completion
            var selectedProviderPane=false
            for _ in 0..<10 {
                guard let element=parent else { break }
                if text(element,"AXIdentifier")=="session-pane-empty" {
                    let label=text(element,"AXDescription")
                    print("PROVIDER_PANE_LABEL",label)
                    selectedProviderPane=label.hasSuffix(", active")
                    break
                }
                parent=value(element,"AXParent").map{$0 as! AXUIElement}
            }
            try check(selectedProviderPane,"Exact attached provider pane is selected after row return")
            try press(root,title:"Work details")
            _ = try byID("workspace-task-directive")
            try press(root,title:"Return to terminals")
            try check(panes().count==4,"Details return retains all four native panes")
            try check(NSDictionary(dictionary:initialClient).isEqual(to:try client()),"Navigation retains exact provider client")
            try check(try shellReceipts()==initialShells,"Navigation retains all four shell PID/birth receipts")
            try press(root,title:"Complete session")
            let completeDeadline=Date(timeIntervalSinceNow:20)
            while try sessions().contains(where:{$0["id"] as? String==id}) && Date()<completeDeadline { settle(0.2) }
            try check(try !sessions().contains(where:{$0["id"] as? String==id}),"Owned Session is resolved through UI")
            try check(kill(providerPID,0) != 0,"Owned provider has exited")
            try check(panes().count==4,"Completion preserves all four shell panes")
            try check(try shellReceipts()==initialShells,"Completion retains original launch shell and companions")
            try save(try shellReceipts(),"shells-after.json")
            print("PASS configured Session row and nested layout retention; no keyboard input or draft assertion")
        } catch { print("FAIL",error); exitStatus=1 }
    }
}
while !finished { RunLoop.current.run(until:Date(timeIntervalSinceNow:0.1)) }
settle(1)
try save(try sessions(),"sessions-after.json")
exit(exitStatus)
