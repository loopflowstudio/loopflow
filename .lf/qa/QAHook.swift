// Copy into swift/LoopflowMac/ for a local QA build; never commit it there. See desktop.md, "Driving the app with the screen locked".
import AppKit

@MainActor
enum QAHook {
    private static var timer: Timer?

    static func start() {
        guard let dir = ProcessInfo.processInfo.environment["LOOPFLOW_QA_HOOK_DIR"] else { return }
        let app = AXUIElementCreateApplication(getpid())
        AXUIElementSetAttributeValue(app, "AXManualAccessibility" as CFString, kCFBooleanTrue)
        AXUIElementSetAttributeValue(app, "AXEnhancedUserInterface" as CFString, kCFBooleanTrue)
        timer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { _ in
            MainActor.assumeIsolated { poll(dir) }
        }
    }

    private static func poll(_ dir: String) {
        let fm = FileManager.default
        let names = ((try? fm.contentsOfDirectory(atPath: dir)) ?? []).filter { $0.hasSuffix(".cmd") }.sorted()
        guard let name = names.first else { return }
        let path = dir + "/" + name
        let line = ((try? String(contentsOfFile: path, encoding: .utf8)) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        try? fm.removeItem(atPath: path)
        let out = run(line)
        try? out.write(toFile: path + ".out", atomically: true, encoding: .utf8)
    }

    private static var windows: [NSWindow] {
        NSApp.windows.filter { $0.isVisible && $0.contentView != nil && $0.frame.width > 40 }
    }

    private static func run(_ line: String) -> String {
        let parts = line.split(separator: " ", maxSplits: 1).map(String.init)
        let arg = parts.count > 1 ? parts[1] : ""
        switch parts.first ?? "" {
        case "snap":
            var lines: [String] = []
            for (index, window) in windows.enumerated() {
                let file = "\(arg)-\(index).png"
                do {
                    _ = try SnapshotService().snapshotWindow(window, to: file)
                    lines.append("\(index) \(window.title) \(Int(window.frame.width))x\(Int(window.frame.height)) \(file)")
                } catch { lines.append("\(index) \(window.title) failed \(error)") }
            }
            return lines.joined(separator: "\n")
        case "tree":
            var lines: [String] = []
            for (index, window) in windows.enumerated() {
                lines.append("window \(index) \(window.title) \(window.frame)")
                walk(window, depth: 1, window: window, into: &lines)
            }
            return lines.joined(separator: "\n")
        case "press":
            for window in windows {
                if let element = find(window, arg) {
                    return element.accessibilityPerformPress() ? "pressed" : "press refused"
                }
            }
            return "not found"
        case "click":
            let n = arg.split(separator: " ").compactMap { Double($0) }
            guard n.count == 3, windows.indices.contains(Int(n[0])) else { return "usage: click window x y" }
            let window = windows[Int(n[0])]
            let height = window.contentView?.frame.height ?? window.frame.height
            let point = NSPoint(x: n[1], y: height - n[2])
            func event(_ type: NSEvent.EventType) -> NSEvent? {
                NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                   windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)
            }
            // The up waits in the queue so an AppKit control's tracking loop finds it.
            if let up = event(.leftMouseUp) { NSApp.postEvent(up, atStart: false) }
            if let down = event(.leftMouseDown) { window.sendEvent(down) }
            return "clicked \(window.title) \(point)"
        case "size":
            let n = arg.split(separator: " ").compactMap { Double($0) }
            guard n.count == 3, windows.indices.contains(Int(n[0])) else { return "usage: size window w h" }
            windows[Int(n[0])].setContentSize(NSSize(width: n[1], height: n[2]))
            return "sized"
        case "menu":
            func search(_ menu: NSMenu) -> NSMenuItem? {
                for item in menu.items {
                    if item.title == arg { return item }
                    if let found = item.submenu.flatMap(search) { return found }
                }
                return nil
            }
            guard let menu = NSApp.mainMenu else { return "no menu" }
            if arg.isEmpty {
                func list(_ menu: NSMenu, _ depth: Int) -> [String] {
                    menu.items.flatMap { [String(repeating: " ", count: depth) + $0.title] + ($0.submenu.map { list($0, depth + 1) } ?? []) }
                }
                return list(menu, 0).joined(separator: "\n")
            }
            guard let item = search(menu), let action = item.action else { return "no item" }
            item.menu?.update()
            return NSApp.sendAction(action, to: item.target, from: item) ? "performed" : "refused"
        case "close":
            guard let index = Int(arg), windows.indices.contains(index) else { return "usage: close window" }
            windows[index].performClose(nil)
            return "closed"
        case "type":
            guard let window = NSApp.keyWindow ?? windows.first else { return "no window" }
            for character in arg {
                for type in [NSEvent.EventType.keyDown, .keyUp] {
                    if let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                    windowNumber: window.windowNumber, context: nil, characters: String(character),
                                                    charactersIgnoringModifiers: String(character), isARepeat: false, keyCode: 0) {
                        NSApp.postEvent(event, atStart: false)
                    }
                }
            }
            return "typed"
        default:
            return "unknown: \(line)"
        }
    }

    private static func children(_ element: Any) -> [Any] {
        if let window = element as? NSWindow { return window.contentView.map { [$0] } ?? [] }
        var result: [Any] = []
        if let node = element as? NSAccessibilityProtocol { result += node.accessibilityChildren() ?? [] }
        if result.isEmpty, let view = element as? NSView { result += view.subviews }
        return result
    }

    private static func walk(_ element: Any, depth: Int, window: NSWindow, into lines: inout [String]) {
        guard depth < 60 else { return }
        for child in children(element) {
            if let node = child as? NSAccessibilityProtocol {
                let role = node.accessibilityRole()?.rawValue ?? "?"
                let id = node.accessibilityIdentifier() ?? ""
                let label = node.accessibilityLabel() ?? ""
                let value = (node.accessibilityValue() as? String) ?? ""
                let title = node.accessibilityTitle() ?? ""
                if !(id.isEmpty && label.isEmpty && value.isEmpty && title.isEmpty) || role == "AXButton" {
                    let screen = node.accessibilityFrame()
                    let x = Int(screen.midX - window.frame.minX)
                    let contentHeight = window.contentView?.frame.height ?? window.frame.height
                    let y = Int(contentHeight - (screen.midY - window.frame.minY))
                    lines.append(String(repeating: " ", count: min(depth, 20)) + "\(role) id=\(id) label=\(label) title=\(title) value=\(value.prefix(80)) @\(x),\(y) \(Int(screen.width))x\(Int(screen.height))")
                }
            }
            walk(child, depth: depth + 1, window: window, into: &lines)
        }
    }

    private static func find(_ element: Any, _ key: String, depth: Int = 0) -> NSAccessibilityProtocol? {
        guard depth < 60 else { return nil }
        for child in children(element) {
            if let node = child as? NSAccessibilityProtocol,
               node.accessibilityIdentifier() == key || node.accessibilityLabel() == key || node.accessibilityTitle() == key {
                return node
            }
            if let found = find(child, key, depth: depth + 1) { return found }
        }
        return nil
    }
}
