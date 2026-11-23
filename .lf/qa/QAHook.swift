// Copy into swift/LoopflowMac/ for a local QA build; never commit it there. See desktop.md, "Driving the app with the screen locked".
import AppKit
import WebKit

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
        case "frame":
            // The whole frame view, title bar and toolbar included, which `snap` leaves out.
            var lines: [String] = []
            for (index, window) in windows.enumerated() {
                guard let frame = window.contentView?.superview,
                      let rep = frame.bitmapImageRepForCachingDisplay(in: frame.bounds) else { continue }
                frame.cacheDisplay(in: frame.bounds, to: rep)
                let file = "\(arg)-\(index).png"
                try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: file))
                lines.append("\(index) \(window.title) \(file)")
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
        case "web":
            // `web <png path>`: WebKit's own snapshot of the first web view, plus its body text and scroll offset.
            // `snap` reads WebKit content reliably only right after a load.
            func webView(_ view: NSView?) -> WKWebView? {
                guard let view else { return nil }
                if let web = view as? WKWebView { return web }
                return view.subviews.lazy.compactMap(webView).first
            }
            guard let web = windows.lazy.compactMap({ webView($0.contentView) }).first else { return "no web view" }
            web.takeSnapshot(with: nil) { image, _ in
                guard let tiff = image?.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff) else { return }
                try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: arg))
            }
            web.evaluateJavaScript("document.body.innerText.slice(0, 120) + ' | scrollY=' + scrollY") { value, error in
                try? String(describing: value ?? error ?? "nil").write(toFile: arg + ".txt", atomically: true, encoding: .utf8)
            }
            return "web \(arg)"
        case "webscroll":
            // `webscroll <y>`: scrolls the first web view as a wheel would, for scroll-restoration checks.
            func webView(_ view: NSView?) -> WKWebView? {
                guard let view else { return nil }
                if let web = view as? WKWebView { return web }
                return view.subviews.lazy.compactMap(webView).first
            }
            guard let web = windows.lazy.compactMap({ webView($0.contentView) }).first else { return "no web view" }
            web.evaluateJavaScript("scrollTo(0, \(Int(arg) ?? 0))")
            return "scrolled"
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
        case "import":
            // Stands in for the file panel the Import tile opens: reports and cancels it, then imports `arg` as the panel's OK does.
            let panels = NSApp.windows.compactMap { $0 as? NSOpenPanel }.filter(\.isVisible)
            panels.forEach { $0.cancel(nil) }
            AppearanceStore.shared.importTheme(URL(fileURLWithPath: arg))
            return "imported (open panels cancelled: \(panels.count))"
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
        case "fill":
            // `fill <placeholder>|<text>`: focuses the text field showing that placeholder and inserts text through its field editor.
            let pieces = arg.split(separator: "|", maxSplits: 1).map(String.init)
            guard pieces.count == 2 else { return "usage: fill placeholder|text" }
            for window in windows {
                guard let field = textField(in: window.contentView, placeholder: pieces[0]) else { continue }
                window.makeFirstResponder(field)
                (window.firstResponder as? NSTextView)?.insertText(pieces[1], replacementRange: NSRange(location: NSNotFound, length: 0))
                return "filled \(window.title)"
            }
            return "no field"
        case "key":
            // `key <window> <mods> <chars|return|escape|up|down>`: a key event sent straight to that window's first responder.
            let pieces = arg.split(separator: " ").map(String.init)
            guard pieces.count == 3, let index = Int(pieces[0]), windows.indices.contains(index) else { return "usage: key window mods chars" }
            let window = windows[index]
            var flags: NSEvent.ModifierFlags = []
            if pieces[1].contains("cmd") { flags.insert(.command) }
            if pieces[1].contains("shift") { flags.insert(.shift) }
            if pieces[1].contains("ctrl") { flags.insert(.control) }
            if pieces[1].contains("opt") { flags.insert(.option) }
            let named: [String: (String, UInt16)] = ["return": ("\r", 36), "escape": ("\u{1b}", 53), "up": (String(UnicodeScalar(NSUpArrowFunctionKey)!), 126),
                                                    "down": (String(UnicodeScalar(NSDownArrowFunctionKey)!), 125), "space": (" ", 49)]
            let (chars, code) = named[pieces[2]] ?? (pieces[2], 0)
            for type in [NSEvent.EventType.keyDown, .keyUp] {
                if let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                                windowNumber: window.windowNumber, context: nil, characters: chars,
                                                charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code) {
                    if type == .keyDown, !window.performKeyEquivalent(with: event) { window.firstResponder?.keyDown(with: event) }
                }
            }
            return "key \(String(describing: window.firstResponder.map { type(of: $0) }))"
        case "responder":
            return windows.map { "\($0.title): \(String(describing: $0.firstResponder.map { type(of: $0) }))" }.joined(separator: "\n")
        default:
            return "unknown: \(line)"
        }
    }

    private static func textField(in view: NSView?, placeholder: String) -> NSTextField? {
        guard let view else { return nil }
        if let field = view as? NSTextField, field.isEditable, field.placeholderString == placeholder { return field }
        for child in view.subviews { if let found = textField(in: child, placeholder: placeholder) { return found } }
        return nil
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
