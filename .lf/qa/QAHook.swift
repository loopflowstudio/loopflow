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
        case "views":
            // The NSView tree with window-space centers, text field values and each clip view's scroll origin.
            guard let index = Int(arg), windows.indices.contains(index), let root = windows[index].contentView?.superview else { return "usage: views window" }
            var lines: [String] = []
            func dump(_ view: NSView, _ depth: Int) {
                guard depth < 40 else { return }
                let box = view.convert(view.bounds, to: nil)
                let height = view.window?.contentView?.frame.height ?? 0
                let clip = (view as? NSClipView).map { " origin=\(Int($0.bounds.origin.y)) document=\(Int($0.documentView?.frame.height ?? 0))" } ?? ""
                let text = (view as? NSTextField).map { " text=\($0.stringValue.prefix(80)) placeholder=\($0.placeholderString ?? "")" } ?? ""
                lines.append(String(repeating: " ", count: depth) + "\(type(of: view)) @\(Int(box.midX)),\(Int(height - box.midY)) \(Int(view.frame.width))x\(Int(view.frame.height))" + clip + text)
                view.subviews.forEach { dump($0, depth + 1) }
            }
            dump(root, 0)
            return lines.joined(separator: "\n")
        case "field":
            // field <placeholder>|<text>: replaces the text of the field with that placeholder; an empty <text> clears it.
            let pieces = arg.split(separator: "|", maxSplits: 1, omittingEmptySubsequences: false).map(String.init)
            guard pieces.count == 2 else { return "usage: field placeholder|text" }
            func search(_ view: NSView) -> NSTextField? {
                if let field = view as? NSTextField, field.isEditable,
                   (field.placeholderString ?? field.placeholderAttributedString?.string) == pieces[0] { return field }
                for child in view.subviews { if let found = search(child) { return found } }
                return nil
            }
            for window in windows {
                guard let root = window.contentView?.superview, let field = search(root) else { continue }
                window.makeFirstResponder(field)
                (window.firstResponder as? NSTextView)?.selectAll(nil)
                (window.firstResponder as? NSTextView)?.insertText(pieces[1], replacementRange: NSRange(location: NSNotFound, length: 0))
                return "typed into \(window.title)"
            }
            return "not found"
        case "return":
            // Return in the field `field` focused; `arg` is the window title.
            guard let window = windows.first(where: { $0.title == arg }) ?? windows.last else { return "no window" }
            (window.firstResponder as? NSTextView)?.insertNewline(nil)
            return "return in \(window.title)"
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
        case "tap":
            // `tap <window> <x> <y> [m|s|c|o…]`: down and up both through the event queue, so local
            // event monitors (the code reader's word clicks) see them in a window that isn't key.
            let n = arg.split(separator: " ").map(String.init)
            guard n.count >= 3, let index = Int(n[0]), windows.indices.contains(index),
                  let x = Double(n[1]), let y = Double(n[2]) else { return "usage: tap window x y [mods]" }
            let window = windows[index]
            let height = window.contentView?.frame.height ?? window.frame.height
            var flags = NSEvent.ModifierFlags()
            for letter in n.count > 3 ? n[3] : "" {
                switch letter {
                case "s": flags.insert(.shift)
                case "c": flags.insert(.control)
                case "o": flags.insert(.option)
                case "m": flags.insert(.command)
                default: break
                }
            }
            for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
                guard let event = NSEvent.mouseEvent(with: type, location: NSPoint(x: x, y: height - y), modifierFlags: flags,
                                                     timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                                                     context: nil, eventNumber: 0, clickCount: 1, pressure: 1) else { return "no event" }
                NSApp.postEvent(event, atStart: false)
            }
            return "tapped \(window.title) \(x),\(y) flags \(flags.rawValue)"
        case "web":
            // `web <png path>`: WebKit's own snapshot of the first web view, plus its body text and scroll offset.
            // `snap` reads WebKit content reliably only right after a load.
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
            guard let web = windows.lazy.compactMap({ webView($0.contentView) }).first else { return "no web view" }
            web.evaluateJavaScript("scrollTo(0, \(Int(arg) ?? 0))")
            return "scrolled"
        case "drag":
            // A SwiftUI DragGesture: down, a few moves, up, each sent straight to the window.
            let n = arg.split(separator: " ").compactMap { Double($0) }
            guard n.count == 5, windows.indices.contains(Int(n[0])) else { return "usage: drag window x y x2 y2" }
            let window = windows[Int(n[0])]
            let height = window.contentView?.frame.height ?? window.frame.height
            func event(_ type: NSEvent.EventType, _ x: Double, _ y: Double) -> NSEvent? {
                NSEvent.mouseEvent(with: type, location: NSPoint(x: x, y: height - y), modifierFlags: [],
                                   timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                                   context: nil, eventNumber: 0, clickCount: 1, pressure: 1)
            }
            event(.leftMouseDown, n[1], n[2]).map(window.sendEvent)
            for step in 1...12 {
                let t = Double(step) / 12
                event(.leftMouseDragged, n[1] + (n[3] - n[1]) * t, n[2] + (n[4] - n[2]) * t).map(window.sendEvent)
            }
            event(.leftMouseUp, n[3], n[4]).map(window.sendEvent)
            return "dragged \(window.title)"
        case "mousedown":
            // `mousedown <window> <x> <y> [<x2> <y2>] [m|s…]`: queues the drag and release, then calls the hit view's
            // own mouseDown. A window that isn't key otherwise eats the first press of an AppKit view that
            // doesn't accept first mouse (the code reader's text and gutter).
            var n = arg.split(separator: " ").map(String.init)
            var flags = NSEvent.ModifierFlags()
            if let last = n.last, Double(last) == nil {
                for letter in last {
                    switch letter {
                    case "s": flags.insert(.shift)
                    case "m": flags.insert(.command)
                    default: break
                    }
                }
                n.removeLast()
            }
            let v = n.compactMap(Double.init)
            guard v.count == 3 || v.count == 5, windows.indices.contains(Int(v[0])) else { return "usage: mousedown window x y [x2 y2] [mods]" }
            let window = windows[Int(v[0])]
            let height = window.contentView?.frame.height ?? window.frame.height
            let (x2, y2) = v.count == 5 ? (v[3], v[4]) : (v[1], v[2])
            func event(_ type: NSEvent.EventType, _ x: Double, _ y: Double) -> NSEvent? {
                NSEvent.mouseEvent(with: type, location: NSPoint(x: x, y: height - y), modifierFlags: flags,
                                   timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                                   context: nil, eventNumber: 0, clickCount: 1, pressure: 1)
            }
            if v.count == 5 {
                for step in 1...12 {
                    let t = Double(step) / 12
                    event(.leftMouseDragged, v[1] + (x2 - v[1]) * t, v[2] + (y2 - v[2]) * t).map { NSApp.postEvent($0, atStart: false) }
                }
            }
            event(.leftMouseUp, x2, y2).map { NSApp.postEvent($0, atStart: false) }
            guard let down = event(.leftMouseDown, v[1], v[2]),
                  let frame = window.contentView?.superview ?? window.contentView,
                  let hit = frame.hitTest(frame.convert(down.locationInWindow, from: nil)) else { return "no view" }
            hit.mouseDown(with: down)
            return "mousedown \(type(of: hit))"
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
        case "windowkey":
            // `windowkey <window> <mods> <chars|return|escape|up|down>`: a key event sent straight to that window's first responder.
            let pieces = arg.split(separator: " ").map(String.init)
            guard pieces.count == 3, let index = Int(pieces[0]), windows.indices.contains(index) else { return "usage: windowkey window mods chars" }
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
            return "windowkey \(String(describing: window.firstResponder.map { type(of: $0) }))"
        case "scrolls":
            var lines: [String] = []
            func visit(_ view: NSView) {
                if let scroll = view as? NSScrollView, let doc = scroll.documentView {
                    lines.append("\(type(of: scroll)) visible=\(scroll.contentView.bounds) doc=\(doc.frame.size) at=\(scroll.convert(scroll.bounds, to: nil))")
                }
                view.subviews.forEach(visit)
            }
            windows.forEach { $0.contentView.map(visit) }
            return lines.joined(separator: "\n")
        case "responder":
            return windows.map { "\($0.title): \(String(describing: $0.firstResponder.map { type(of: $0) }))" }.joined(separator: "\n")
        case "equivalents":
            // Lists main-menu items with a key equivalent: title, characters (hex), modifier mask, enabled, hidden.
            func list(_ menu: NSMenu) -> [String] {
                menu.items.flatMap { item -> [String] in
                    let own = item.keyEquivalent.isEmpty ? [] : ["\(item.title) \(item.keyEquivalent.unicodeScalars.map { String($0.value, radix: 16) }) \(item.keyEquivalentModifierMask.rawValue) enabled=\(item.isEnabled) hidden=\(item.isHidden)"]
                    return own + (item.submenu.map(list) ?? [])
                }
            }
            return NSApp.mainMenu.map(list)?.joined(separator: "\n") ?? "no menu"
        case "claims":
            // Who claims ⌘↩ as a key equivalent: the main menu, then the window's views.
            guard let window = windows.first(where: { $0.firstResponder is GhosttyMetalView }),
                  let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: window.windowNumber, context: nil, characters: "\r",
                                               charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36) else { return "no terminal" }
            // AppKit asks the key window first; a locked screen has no key window, so ask it here.
            if window.performKeyEquivalent(with: event) { return "window" }
            let claimed = (NSApp.mainMenu?.items ?? []).filter { $0.submenu?.performKeyEquivalent(with: event) == true }.map(\.title)
            return "menus \(claimed)"
        case "terminal":
            // Makes the first terminal pane first responder and prints its screen.
            for window in windows {
                if let view = terminalView(window.contentView) {
                    window.makeFirstResponder(view)
                    if case .available(let text, _) = try? view.readText(region: .screen, maxBytes: 65536) { return text }
                    return "no screen (accepts input: \(view.connectionAcceptsInput))"
                }
            }
            return "no terminal"
        case "key":
            // key <post|send> <keyCode> <flags: s c o m, plus raw device bits like o+0x20> <characters> [unmodified]
            // post goes through the app's event queue (local monitors, menus, performKeyEquivalent); send skips monitors.
            let n = arg.split(separator: " ", omittingEmptySubsequences: false).map(String.init)
            guard n.count >= 4, let code = UInt16(n[1]),
                  let window = windows.first(where: { $0.firstResponder is GhosttyMetalView }) ?? windows.first else {
                return "usage: key post|send code flags chars [unmodified]"
            }
            var flags = NSEvent.ModifierFlags()
            for part in n[2].split(separator: "+") {
                if part.hasPrefix("0x"), let raw = UInt(part.dropFirst(2), radix: 16) { flags.insert(NSEvent.ModifierFlags(rawValue: raw)); continue }
                for letter in part {
                    switch letter {
                    case "s": flags.insert(.shift)
                    case "c": flags.insert(.control)
                    case "o": flags.insert(.option)
                    case "m": flags.insert(.command)
                    default: break
                    }
                }
            }
            func decode(_ text: String) -> String {
                text.replacingOccurrences(of: "\\r", with: "\r").replacingOccurrences(of: "\\t", with: "\t").replacingOccurrences(of: "\\s", with: " ")
            }
            let chars = decode(n[3]), plain = n.count > 4 ? decode(n[4]) : chars
            for type in [NSEvent.EventType.keyDown, .keyUp] {
                guard let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                                   windowNumber: window.windowNumber, context: nil, characters: chars,
                                                   charactersIgnoringModifiers: plain, isARepeat: false, keyCode: code) else { return "no event" }
                if n[0] == "post" { NSApp.postEvent(event, atStart: false) } else { window.sendEvent(event) }
            }
            return "\(n[0]) \(code) \(flags.rawValue) to \(window.title) first responder \(String(describing: window.firstResponder.map { type(of: $0) }))"
        default:
            return "unknown: \(line)"
        }
    }

    private static func webView(_ view: NSView?) -> WKWebView? {
        guard let view else { return nil }
        if let web = view as? WKWebView { return web }
        return view.subviews.lazy.compactMap(webView).first
    }

    private static func terminalView(_ view: NSView?) -> GhosttyMetalView? {
        guard let view else { return nil }
        if let terminal = view as? GhosttyMetalView, terminal.surface != nil { return terminal }
        for child in view.subviews { if let found = terminalView(child) { return found } }
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
