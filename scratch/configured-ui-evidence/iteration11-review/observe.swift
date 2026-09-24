import AppKit
import ApplicationServices
print("observed_at", ISO8601DateFormatter().string(from: Date()))
print("ax_trusted", AXIsProcessTrusted())
func attr(_ node: AXUIElement, _ key: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(node, key as CFString, &value) == .success else { return nil }
    return value
}
var visited = 0
func inspect(_ node: AXUIElement, _ depth: Int = 0) {
    guard depth < 35, visited < 4000 else { return }; visited += 1
    let id = attr(node, kAXIdentifierAttribute) as? String ?? ""
    if (id.hasPrefix("workspace-task-") && id != "workspace-task-directive") || id.hasPrefix("session-row-") || id == "workspace-task-view" || id == "podium-sessions" {
        print("control", id, "value", attr(node, kAXValueAttribute) as? String ?? "")
    }
    for child in attr(node, kAXChildrenAttribute) as? [AXUIElement] ?? [] { inspect(child, depth + 1) }
}
for app in NSWorkspace.shared.runningApplications where app.bundleIdentifier == "studio.loopflow.review.active" {
    visited = 0
    let node = AXUIElementCreateApplication(app.processIdentifier)
    let windows = attr(node, kAXWindowsAttribute) as? [AXUIElement] ?? []
    print("app", app.processIdentifier, "active", app.isActive, "windows", windows.count)
    for window in windows { inspect(window) }
    print("nodes", visited)
}
