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

func focusedPID() -> pid_t {
    var focused: CFTypeRef?
    let status=AXUIElementCopyAttributeValue(AXUIElementCreateSystemWide(),kAXFocusedApplicationAttribute as CFString,&focused)
    guard status == .success,let focused else { print("Focused application status",status.rawValue); return -1 }
    var pid: pid_t = -1
    AXUIElementGetPid(focused as! AXUIElement,&pid)
    return pid
}
setbuf(stdout, nil)
let proofRoot = "/tmp/loo291-review10"
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
            try check(focusedPID() == running.processIdentifier, "Owned app focused before workspace trial")
            if let field = elements(root).first(where: { text($0, "AXIdentifier") == "workspace-search" && text($0, "AXRole") == "AXTextField" }) {
                print("Search reset", AXUIElementSetAttributeValue(field, kAXValueAttribute as CFString, "LOO-291" as CFTypeRef).rawValue)
            }
            let taskId = "workspace-task-ee671927-255f-41e6-8429-b830d59cc1de"
            let ready = ProcessInfo.processInfo.systemUptime + 20
            while !elements(root).contains(where: { text($0,"AXIdentifier") == taskId }) && ProcessInfo.processInfo.systemUptime < ready { settle(0.1) }
            if !elements(root).contains(where: { text($0,"AXIdentifier") == taskId }) {
                for e in elements(root) { print("BOUNDARY",text(e,"AXRole"),text(e,"AXIdentifier"),text(e,"AXTitle"),String(text(e,"AXValue").prefix(500))) }
            }
            func send(_ input:String,enter:Bool=false) throws {
                if !running.isActive {
                    print("Input foreground",NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "unknown")
                    print("Input activation",running.activate(options:[.activateAllWindows])); settle(0.8)
                }
                print("Foreground identity",NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1,"owned",running.processIdentifier,"cached active",running.isActive,"AX focus",focusedPID())
                try check(focusedPID() == running.processIdentifier,"Owned app foreground for input")
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
            try press(root,title:"All work")
            try press(root,title:"New terminal")
            settle(1)
            try send("printf '%s\\n' $$ > /tmp/loo291-review10/main-shell.pid",enter:true)
            try press(root,title:"Show work list")
            let target = try find(root,id:taskId)
            print("TIMING launch_to_task_ax_ms",(ProcessInfo.processInfo.systemUptime-launchedAt)*1000)
            try check(AXUIElementPerformAction(target,kAXPressAction as CFString) == .success,"Select exact Task")
            settle()
            if !running.isActive {
                print("Launch foreground",NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "unknown")
                print("Launch activation",running.activate(options:[.activateAllWindows])); settle(0.8)
            }
            print("Foreground identity",NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1,"owned",running.processIdentifier,"cached active",running.isActive,"AX focus",focusedPID())
            try check(focusedPID() == running.processIdentifier,"Owned app foreground before launch")
            let start = ProcessInfo.processInfo.systemUptime
            try press(root,title:"New conversation · LOO-291")
            var owned: [String: Any]?
            let deadline = ProcessInfo.processInfo.systemUptime + 60
            while owned == nil && ProcessInfo.processInfo.systemUptime < deadline {
                owned = try sessions().first { row in
                    guard let id=row["id"] as? String, !prior.contains(id), let work=row["work"] as? [String:Any] else { return false }
                    return work["id"] as? String == "task_2aa71a7e36fe416d8a721e2b2f7c54e7"
                }
                if owned == nil { settle(0.5) }
            }
            guard let record=owned, let id=record["id"] as? String else { throw Failure(message:"No exact new Task Session observed") }
            try save(record,"owned-session.json")
            print("Owned Session",id,"terminal_ids",record["terminal_ids"] ?? [])
            try check(!(record["terminal_ids"] as? [String] ?? []).isEmpty,"Shared projection records shell attachment")
            let dir=controlHome + "/runs/" + String(id.dropFirst(4).prefix(2)) + "/" + id
            func client() throws -> [String:Any] {
                let files=try FileManager.default.contentsOfDirectory(atPath:dir + "/provider-clients")
                let clients=try files.filter { $0.hasSuffix(".json") }.map { try json(dir+"/provider-clients/"+$0) }.filter { kill(($0["pid"] as? NSNumber)?.int32Value ?? 0,0)==0 }
                guard clients.count==1 else { throw Failure(message:"Expected one live owned provider") }
                return clients[0]
            }
            let initialClient=try client()
            guard let providerPid=(initialClient["pid"] as? NSNumber)?.int32Value else { throw Failure(message:"No client pid") }
            var ancestor=providerPid
            for _ in 0..<12 {
                if ancestor==running.processIdentifier || ancestor<=1 { break }
                ancestor=Int32(String(decoding:try run("/bin/ps",["-p",String(ancestor),"-o","ppid="]),as:UTF8.self).trimmingCharacters(in:.whitespacesAndNewlines)) ?? 0
            }
            try check(ancestor==running.processIdentifier,"Provider descends from this owned app")
            try save(initialClient,"client-before.json")
            let provider=try json(dir+"/provider-session.json")
            let conversation=provider["provider_session_id"] as! String
            let historyRoot="/Users/jack/.lf/accounts/codex/engineering/sessions"
            guard let walker=FileManager.default.enumerator(atPath:historyRoot),
                  let relative=walker.allObjects.compactMap({$0 as? String}).first(where:{$0.hasSuffix(conversation+".jsonl")}) else { throw Failure(message:"Exact provider history unavailable") }
            let historyPath=historyRoot+"/"+relative
            try save(["run":id,"conversation":conversation,"history_path":historyPath],"history-ref.json")
            func messages(_ role: String) throws -> [String] {
                try String(contentsOfFile:historyPath,encoding:.utf8).split(separator:"\n").compactMap { line in
                    guard let data=String(line).data(using:.utf8), let row=try? JSONSerialization.jsonObject(with:data) as? [String:Any],row["type"] as? String == "response_item",let payload=row["payload"] as? [String:Any],payload["role"] as? String == role,let content=payload["content"] as? [[String:Any]] else { return nil }
                    return content.compactMap{$0["text"] as? String}.joined()
                }
            }
            let initialReplyDeadline=ProcessInfo.processInfo.systemUptime+60
            while try messages("assistant").isEmpty && ProcessInfo.processInfo.systemUptime<initialReplyDeadline { settle(0.3) }
            try check(try !messages("assistant").isEmpty,"Configured provider produced initial response")
            print("TIMING new_conversation_to_initial_response_ms",(ProcessInfo.processInfo.systemUptime-start)*1000)
            try press(root,title:"Show work list")
            let rowDeadline=ProcessInfo.processInfo.systemUptime+20
            while !elements(root).contains(where:{text($0,"AXIdentifier")=="session-row-"+id}) && ProcessInfo.processInfo.systemUptime<rowDeadline { settle(0.2) }
            let row=try find(root,id:"session-row-"+id)
            try check(AXUIElementPerformAction(row,kAXPressAction as CFString) == .success,"Select exact new Session row into originating shell")
            settle(0.5)
            try check(NSDictionary(dictionary:initialClient).isEqual(to:try client()),"Row selection preserves exact provider client")
            try check(!elements(root).contains{ text($0,"AXDescription")=="Move here" },"Local row does not request transfer")
            try press(root,title:"New terminal")
            settle(1)
            try send("printf '%s\\n' $$ > /tmp/loo291-review10/task-companion.pid",enter:true)
            try press(root,title:"Split worktrees right")
            guard let picker=elements(root).first(where:{text($0,"AXRole")=="AXMenuButton" && text($0,"AXDescription")=="Choose worktree"}) else { throw Failure(message:"Missing outer picker") }
            try check(AXUIElementPerformAction(picker,kAXShowMenuAction as CFString) == .success,"Open outer worktree picker")
            settle(0.3)
            for e in elements(root).filter({text($0,"AXRole")=="AXMenuItem"}) { print("WORKTREE_OPTION",text(e,"AXTitle"),text(e,"AXDescription")) }
            guard let main=elements(root).first(where:{text($0,"AXRole")=="AXMenuItem" && text($0,"AXTitle")=="loopflow"}) else { throw Failure(message:"Missing retained main checkout") }
            try check(AXUIElementPerformAction(main,kAXPressAction as CFString) == .success,"Restore main checkout beside Task")
            settle(0.5)
            try press(root,title:"New terminal")
            settle(1)
            try send("printf '%s\\n' $$ > /tmp/loo291-review10/main-companion.pid",enter:true)
            let panes=elements(root).filter{text($0,"AXIdentifier")=="pane-split-right"}
            try check(panes.count==4,"Two checkout groups retain two inner panes each")
            let currentRow=try find(root,id:"session-row-"+id)
            try check(AXUIElementPerformAction(currentRow,kAXPressAction as CFString) == .success,"Return from other checkout to exact Session")
            settle(0.5)
            try check(elements(root).filter{text($0,"AXIdentifier")=="pane-split-right"}.count==4,"Session return retains both nested layouts")
            let sentence="Do not use tools or edit files. Reply only LOO291_REVIEW10_ROW_RETURN."
            try send("Do not use tools or edit files. ")
            try press(root,title:"Work details")
            try press(root,title:"All work")
            try press(root,title:"Return to terminals")
            let currentClient=try client()
            try check(NSDictionary(dictionary:initialClient).isEqual(to:currentClient),"Same exact client through navigation")
            try send("Reply only LOO291_REVIEW10_ROW_RETURN.",enter:true)
            let replyDeadline=ProcessInfo.processInfo.systemUptime+60
            while try !messages("assistant").contains("LOO291_REVIEW10_ROW_RETURN") && ProcessInfo.processInfo.systemUptime<replyDeadline { settle(0.3) }
            let user=try messages("user").last ?? ""
            let answered=try messages("assistant").contains("LOO291_REVIEW10_ROW_RETURN")
            try save(["last_user":user,"answered":answered],"messages.json")
            try check(user==sentence,"Exact draft survives integrated navigation")
            try check(answered,"Same provider replied after navigation")
            try check(elements(root).filter{text($0,"AXRole")=="AXButton" && text($0,"AXDescription")=="Complete session"}.count==1,"One exact local completion control")
            try press(root,title:"Complete session")
            let completionDeadline=ProcessInfo.processInfo.systemUptime+15
            while try sessions().contains(where:{$0["id"] as? String == id}) && ProcessInfo.processInfo.systemUptime<completionDeadline { settle(0.2) }
            try check(try !sessions().contains(where:{$0["id"] as? String == id}),"Shared Session resolved")
            try check(kill(providerPid,0) != 0,"Owned provider exited")
            try send("printf 'LOO291_SHELL_RETURNED\\n' > /tmp/loo291-review10/shell-returned.txt",enter:true)
            settle(1)
            try check((try? String(contentsOfFile:proofRoot+"/shell-returned.txt",encoding:.utf8))=="LOO291_SHELL_RETURNED\n","Original launch pane remains a usable shell")
            try press(root,title:"Work details")
            try check(elements(root).contains{text($0,"AXValue").contains("No open Sessions for this Work")},"Task survives with explicit no-Session state")
            for name in ["main-shell","task-companion","main-companion"] {
                let pid=Int32(try String(contentsOfFile:proofRoot+"/"+name+".pid",encoding:.utf8).trimmingCharacters(in:.whitespacesAndNewlines)) ?? 0
                try check(pid>1 && kill(pid,0)==0,"Retained shell alive: "+name)
            }
            print("PASS configured row return, nested layouts, exact draft and completion")
        } catch { print("FAIL",error); exitStatus=1 }
    }
}
while !finished { RunLoop.current.run(until:Date(timeIntervalSinceNow:0.1)) }
exit(exitStatus)
