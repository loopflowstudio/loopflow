import AppKit
import Foundation
func log(_ value: [String: Any]) {
 let path = "/tmp/loo426-cmux-probe/events.jsonl"
 let data = try! JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) + Data([10])
 if !FileManager.default.fileExists(atPath: path) { FileManager.default.createFile(atPath: path, contents: nil) }
 let f = try! FileHandle(forWritingTo: URL(fileURLWithPath: path)); try! f.seekToEnd(); try! f.write(contentsOf: data); try! f.close()
}
final class Receiver: NSObject, NSApplicationDelegate {
 func applicationWillFinishLaunching(_ notification: Notification) { NSAppleEventManager.shared().setEventHandler(self, andSelector: #selector(receive(_:reply:)), forEventClass: 0x4755524c, andEventID: 0x4755524c) }
 func applicationDidFinishLaunching(_ notification: Notification) {
 let keys = ["TERM_PROGRAM","TERM_PROGRAM_VERSION","CMUX_WORKSPACE_ID","CMUX_SURFACE_ID","CMUX_SOCKET_PATH","CMUX_PANE_ID","HRDR_SESSION_ID","HERDR_SESSION_ID","HRDR_SOCKET","HERDR_ENV","HERDR_SOCKET_PATH","HERDR_WORKSPACE_ID","HERDR_TAB_ID","HERDR_PANE_ID","PWD"]
 log(["kind":"startup", "pid":ProcessInfo.processInfo.processIdentifier,"ppid":getppid(),"environment":ProcessInfo.processInfo.environment.filter { keys.contains($0.key) }])
 }
 @objc func receive(_ event: NSAppleEventDescriptor, reply: NSAppleEventDescriptor) {
 let pid = event.attributeDescriptor(forKeyword: 0x73706964)?.int32Value ?? -1
 let app = NSRunningApplication(processIdentifier: pid)
 log(["environment":ProcessInfo.processInfo.environment.filter { ["TERM_PROGRAM","TERM_PROGRAM_VERSION","CMUX_WORKSPACE_ID","CMUX_SURFACE_ID","CMUX_SOCKET_PATH","HERDR_ENV","HERDR_SOCKET_PATH","HERDR_WORKSPACE_ID","HERDR_TAB_ID","HERDR_PANE_ID","PWD"].contains($0.key) },"kind":"url","url":event.paramDescriptor(forKeyword: 0x2d2d2d2d)?.stringValue ?? "", "sender_pid":pid,"sender_bundle":app?.bundleIdentifier ?? "", "sender_path":app?.executableURL?.path ?? "", "event":event.description])
 }
}
let app = NSApplication.shared
let receiver = Receiver()
app.delegate = receiver
app.setActivationPolicy(.accessory)
app.run()
