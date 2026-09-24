#if os(macOS)
import AppKit
import Foundation
import Testing
#if canImport(GhosttyKit)
import GhosttyKit
#endif
@testable import LoopflowMac
@testable import Loopflow

@Suite("Embedded terminal input")
struct GhosttyTerminalInputTests {
    // SwiftPM links GhosttyKit; the Xcode compile-check target builds the fallback.
#if canImport(GhosttyKit)
    @Test("Shell-launched sessions focus their live terminal and external clients stay elsewhere")
    @MainActor
    func shellSessionAttachment() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: NSTemporaryDirectory())
        let store = SessionsStore(repoPath: NSTemporaryDirectory(), surfaces: registry.surfaces)
        var views: [GhosttyMetalView] = []
        defer { for view in views { view.handleSurfaceClose() } }
        var records: [SessionRecord] = []
        for id in ["one", "two"] {
            workspace.multiplexer.newShell()
            let pane = workspace.multiplexer.focusedPaneId
            let view = registry.surfaces.view(for: .shell(pane))
            view.frame = CGRect(x: 0, y: 0, width: 600, height: 300)
            view.workingDirectory = NSTemporaryDirectory()
            view.command = buildWorkspaceShellCommand(id: pane, argv: ["/bin/sh", "-c", "printf 'attachment-ready\\n'; exec /bin/cat"], env: [:])
            view.createSurface(manager: manager)
            views.append(view)
            _ = try #require(view.surface)
            let data = try JSONSerialization.data(withJSONObject: [
                "id": id, "kind": "interactive", "work": NSNull(), "title": id,
                "detail": "test", "cwd": NSTemporaryDirectory(), "state": "active",
                "ready_summary": NSNull(), "terminal_ids": [pane], "open_argv": ["unused"],
            ])
            records.append(try JSONDecoder().decode(SessionRecord.self, from: data))
        }
        store.reconcile(records)
        for record in records {
            #expect(store.sessions.first { $0.id == record.id }?.state == .live)
            await store.select(record.id)
            #expect(store.sessions.first { $0.id == record.id }?.surface == record)
            #expect(store.localTerminal(for: record) == .shell(record.terminalIds[0]))
        }
        let otherWindow = SessionsStore(repoPath: NSTemporaryDirectory())
        otherWindow.reconcile(records)
        #expect(otherWindow.sessions.allSatisfy { $0.state == .elsewhere })
        await otherWindow.select("one")
        #expect(otherWindow.sessions.first { $0.id == "one" }?.state == .elsewhere)
        #expect(registry.surfaces.hasSurface(.shell(records[0].terminalIds[0])))
    }

    @Test("Exiting the initial conversation leaves a usable companion shell")
    @MainActor
    func conversationReturnsToShell() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let view = GhosttyMetalView(terminal: .shell("return-proof"), frame: CGRect(x: 0, y: 0, width: 800, height: 500))
        view.workingDirectory = NSTemporaryDirectory()
        view.command = buildWorkspaceShellCommand(id: "return-proof", argv: ["/bin/true"], env: [:])
        view.createSurface(manager: manager)
        defer { view.handleSurfaceClose() }
        let surface = try #require(view.surface)
        let command = "printf '%s:%s\\n' companion \"$LF_TERMINAL_ID\"\r"
        command.withCString { ghostty_surface_text(surface, $0, UInt(command.utf8.count)) }
        let deadline = ContinuousClock.now + .seconds(5)
        while !terminalText(view).contains("companion:return-proof"), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(terminalText(view).contains("companion:return-proof"))
    }

    @Test("block clicks copy command and output, then the live prompt clears selection")
    @MainActor
    func commandBlockClickAndCopy() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let window = NSWindow(
            contentRect: CGRect(x: 0, y: 0, width: 600, height: 300),
            styleMask: [.titled], backing: .buffered, defer: false
        )
        let view = GhosttyMetalView(terminal: .shell("block-copy-proof"), frame: window.contentLayoutRect)
        window.contentView = view
        view.workingDirectory = NSTemporaryDirectory()
        // Feed known OSC 133 boundaries through a real PTY and Ghostty parser.
        view.command = #"/bin/sh -c 'printf "\033]133;A\007$ \033]133;B\007echo alpha\r\n\033]133;C\007alpha\r\n\033]133;D;0\007\033]133;A\007$ \033]133;B\007echo beta\r\n\033]133;C\007beta\r\n\033]133;D;0\007\033]133;A\007$ \033]133;B\007ready"; exec /bin/cat'"#
        view.createSurface(manager: manager)
        defer { view.handleSurfaceClose() }
        let surface = try #require(view.surface)
        var blocks = Array(repeating: ghostty_command_block_s(), count: 10)
        var count = 0
        let deadline = ContinuousClock.now + .seconds(3)
        repeat {
            count = blocks.withUnsafeMutableBufferPointer {
                ghostty_surface_command_blocks(surface, $0.baseAddress, $0.count)
            }
            if count == 2 { break }
            try await Task.sleep(for: .milliseconds(20))
        } while ContinuousClock.now < deadline
        try #require(count == 2)
        view.setFrameSize(view.frame.size)
        let size = ghostty_surface_size(surface)
        let cellHeight = CGFloat(size.cell_height_px) / window.backingScaleFactor
        let topInset = max(0, (view.bounds.height - CGFloat(size.rows) * cellHeight) / 2)
        let copy = try #require(NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: .command,
            timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: "c", charactersIgnoringModifiers: "c", isARepeat: false, keyCode: 8
        ))
        let pasteboard = NSPasteboard.general
        let savedItems = (pasteboard.pasteboardItems ?? []).map { item in
            let saved = NSPasteboardItem()
            for type in item.types {
                if let data = item.data(forType: type) { saved.setData(data, forType: type) }
            }
            return saved
        }
        defer {
            pasteboard.clearContents()
            pasteboard.writeObjects(savedItems)
        }
        let clicks: [(UInt16, String?)] = [
            (blocks[0].start_row, "echo alpha\nalpha"),
            (blocks[0].end_row, "echo alpha\nalpha"),
            (blocks[1].end_row, "echo beta\nbeta"),
            (blocks[1].end_row + 1, nil),
        ]
        for (row, expected) in clicks {
            let point = CGPoint(
                x: view.bounds.midX,
                y: view.bounds.height - topInset - (CGFloat(row) + 0.5) * cellHeight
            )
            for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
                let event = try #require(NSEvent.mouseEvent(
                    with: type, location: view.convert(point, to: nil), modifierFlags: [],
                    timestamp: ProcessInfo.processInfo.systemUptime,
                    windowNumber: window.windowNumber, context: nil,
                    eventNumber: 1, clickCount: 1, pressure: 1
                ))
                if type == .leftMouseDown { view.mouseDown(with: event) }
                else { view.mouseUp(with: event) }
            }
            pasteboard.clearContents()
            #expect(view.performKeyEquivalent(with: copy) == (expected != nil))
            #expect(pasteboard.string(forType: .string) == expected)
            #expect(!ghostty_surface_has_selection(surface))
        }
    }

    @Test("releasing a retained terminal affects only its owning window")
    @MainActor
    func releaseSurfaceIsWindowLocal() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let firstPool = GhosttySurfacePool()
        let secondPool = GhosttySurfacePool()
        let id = TerminalIdentity.session("window-isolation")
        let first = firstPool.view(for: id)
        let second = secondPool.view(for: id)
        for view in [first, second] {
            view.setFrameSize(CGSize(width: 400, height: 300))
            view.workingDirectory = NSTemporaryDirectory()
            view.command = "/bin/cat"
            view.createSurface(manager: manager)
            _ = try #require(view.surface)
        }
        defer {
            first.handleSurfaceClose()
            second.handleSurfaceClose()
        }

        firstPool.release(id)
        #expect(first.surface == nil)
        #expect(!firstPool.hasSurface(id))
        #expect(secondPool.hasSurface(id))
        let survivor = try #require(second.surface)
        let marker = "still-running"
        marker.withCString { ghostty_surface_text(survivor, $0, UInt(marker.utf8.count)) }
        let deadline = ContinuousClock.now + .seconds(3)
        while !terminalText(second).contains(marker), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(terminalText(second).contains(marker))
        #expect(firstPool.view(for: id) !== first)
    }

    @Test("terminal title events retain the originating Session identity")
    @MainActor
    func titleEventIdentifiesItsSurface() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let view = GhosttyMetalView(terminal: .session("title-proof"), frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        view.workingDirectory = NSTemporaryDirectory()
        view.command = "/bin/sh -c 'printf \"\\033]2;title-proof\\007title-ready\"; exec /bin/cat'"

        try await confirmation("title delivered for its Session") { confirmed in
            let observer = NotificationCenter.default.addObserver(
                forName: .ghosttyTerminalTitle, object: nil, queue: nil
            ) { notification in
                guard let title = notification.object as? GhosttyTerminalTitle,
                      title.title == "title-proof"
                else { return }
                #expect(title.terminal == .session("title-proof"))
                confirmed()
            }
            defer { NotificationCenter.default.removeObserver(observer) }
            view.createSurface(manager: manager)
            defer { view.handleSurfaceClose() }
            _ = try #require(view.surface)
            let deadline = ContinuousClock.now + .seconds(3)
            while !terminalText(view).contains("title-ready"), ContinuousClock.now < deadline {
                manager.tick()
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(terminalText(view).contains("title-ready"))
            manager.tick()
            await Task.yield()
        }
    }

    @Test("paste shortcuts reach only the focused terminal in a split")
    @MainActor
    func pasteShortcutFollowsFirstResponder() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        #expect(manager.state == .ready)
        let window = NSWindow(
            contentRect: CGRect(x: 0, y: 0, width: 800, height: 300),
            styleMask: [.titled], backing: .buffered, defer: false
        )
        let root = NSView(frame: window.contentLayoutRect)
        window.contentView = root
        let left = GhosttyMetalView(terminal: .session("left"), frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        let right = GhosttyMetalView(terminal: .shell("right"), frame: CGRect(x: 400, y: 0, width: 400, height: 300))
        for view in [left, right] {
            root.addSubview(view)
            view.workingDirectory = NSTemporaryDirectory()
            view.command = "/bin/cat"
            view.createSurface(manager: manager)
            _ = try #require(view.surface)
        }
        defer {
            left.handleSurfaceClose()
            right.handleSurfaceClose()
        }
        let pasteboard = NSPasteboard.general
        let savedItems = (pasteboard.pasteboardItems ?? []).map { item in
            let saved = NSPasteboardItem()
            for type in item.types {
                if let data = item.data(forType: type) { saved.setData(data, forType: type) }
            }
            return saved
        }
        defer {
            pasteboard.clearContents()
            pasteboard.writeObjects(savedItems)
        }
        for (focused, other, marker) in [(right, left, "right-target"), (left, right, "left-target")] {
            #expect(window.makeFirstResponder(focused))
            pasteboard.clearContents()
            pasteboard.setString(marker, forType: .string)
            let event = try #require(NSEvent.keyEvent(
                with: .keyDown, location: .zero, modifierFlags: .command,
                timestamp: 0, windowNumber: window.windowNumber, context: nil,
                characters: "v", charactersIgnoringModifiers: "v", isARepeat: false, keyCode: 9
            ))
            #expect(root.performKeyEquivalent(with: event))
            let deadline = ContinuousClock.now + .seconds(3)
            while !terminalText(focused).contains(marker), ContinuousClock.now < deadline {
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(terminalText(focused).contains(marker))
            #expect(!terminalText(other).contains(marker))
        }
    }

    @MainActor
    private func terminalText(_ view: GhosttyMetalView) -> String {
        guard let surface = view.surface else { return "" }
        var text = ghostty_text_s()
        let selection = ghostty_selection_s(
            top_left: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
            bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0),
            rectangle: false
        )
        guard ghostty_surface_read_text(surface, selection, &text) else { return "" }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text else { return "" }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }

    @Test("bundled shell integration emits semantic command boundaries")
    func shellIntegrationEmitsSemanticMarks() throws {
        let resources = try #require(GhosttyRuntimeResources.directoryURL)
        let home = FileManager.default.temporaryDirectory
            .appendingPathComponent("ghostty-shell-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: home, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: home) }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/zsh")
        process.arguments = [
            "-i",
            "-c",
            """
            _ghostty_deferred_init
            print -Pn "$PS1"
            _ghostty_preexec "print block-proof"
            print block-proof
            _ghostty_precmd
            print -Pn "$PS1"
            """,
        ]
        var environment = ProcessInfo.processInfo.environment
        environment["HOME"] = home.path
        environment["GHOSTTY_RESOURCES_DIR"] = resources.path
        environment["GHOSTTY_SHELL_FEATURES"] = ""
        environment["GHOSTTY_ZSH_ZDOTDIR"] = home.path
        environment["TERM"] = "xterm-256color"
        environment["ZDOTDIR"] = resources
            .appendingPathComponent("shell-integration/zsh", isDirectory: true)
            .path
        process.environment = environment

        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr
        try process.run()
        process.waitUntilExit()

        let output = String(
            decoding: stdout.fileHandleForReading.readDataToEndOfFile(),
            as: UTF8.self
        )
        let error = String(
            decoding: stderr.fileHandleForReading.readDataToEndOfFile(),
            as: UTF8.self
        )
        #expect(process.terminationStatus == 0, "zsh failed: \(error)")

        let markers = [
            "\u{1B}]133;A;cl=line\u{7}",
            "\u{1B}]133;B\u{7}",
            "\u{1B}]133;C\u{7}",
            "block-proof",
            "\u{1B}]133;D;0\u{7}",
        ]
        var remaining = output[...]
        for marker in markers {
            let range = try #require(remaining.range(of: marker))
            remaining = remaining[range.upperBound...]
        }
    }

    @Test("a click anywhere in a command or its output resolves the whole block")
    func commandBlockHitTesting() throws {
        let block = GhosttyCommandBlockLayout(id: 7, startRow: 2, endRow: 5)
        let blocks = [block]

        #expect(ghosttyCommandBlock(atViewportRow: 2, in: blocks) == block)
        #expect(ghosttyCommandBlock(atViewportRow: 4, in: blocks) == block)
        #expect(ghosttyCommandBlock(atViewportRow: 5, in: blocks) == block)
        #expect(ghosttyCommandBlock(atViewportRow: 1, in: blocks) == nil)
        #expect(ghosttyCommandBlock(atViewportRow: 6, in: blocks) == nil)
    }

    @Test("command blocks occupy full-width, row-aligned terminal surfaces")
    func commandBlockLayout() throws {
        let bounds = CGRect(x: 0, y: 0, width: 300, height: 200)
        let block = GhosttyCommandBlockLayout(id: 7, startRow: 1, endRow: 3)
        let frame = ghosttyCommandBlockFrame(
            block,
            bounds: bounds,
            rows: 10,
            cellHeight: 18
        )

        #expect(frame == CGRect(x: 4, y: 119, width: 292, height: 52))
        #expect(ghosttyViewportRow(
            at: CGPoint(x: 150, y: 150),
            bounds: bounds,
            rows: 10,
            cellHeight: 18
        ) == 2)
        #expect(ghosttyViewportRow(
            at: CGPoint(x: 150, y: 195),
            bounds: bounds,
            rows: 10,
            cellHeight: 18
        ) == nil)
    }

    @Test("clicking a split terminal makes that exact pane the input target")
    @MainActor
    func pointerFocusFollowsClickedPane() throws {
        let window = NSWindow(
            contentRect: CGRect(x: 0, y: 0, width: 400, height: 200),
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        let root = NSView(frame: window.contentLayoutRect)
        let left = GhosttyMetalView(terminal: .session("left"), frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        let right = GhosttyMetalView(terminal: .shell("right"), frame: CGRect(x: 200, y: 0, width: 200, height: 200))
        var focusedPane = "left"
        left.onFocus = { focusedPane = "left" }
        right.onFocus = { focusedPane = "right" }
        root.addSubview(left)
        root.addSubview(right)
        window.contentView = root
        #expect(window.makeFirstResponder(left))

        let event = try #require(NSEvent.mouseEvent(
            with: .leftMouseDown,
            location: CGPoint(x: 300, y: 100),
            modifierFlags: [],
            timestamp: 0,
            windowNumber: window.windowNumber,
            context: nil,
            eventNumber: 1,
            clickCount: 1,
            pressure: 1
        ))
        right.mouseDown(with: event)

        #expect(focusedPane == "right")
        #expect(window.firstResponder === right)
    }

    @Test("pasting screenshot bytes gives the provider a readable PNG path")
    func imagePasteCreatesFile() throws {
        let bitmap = try #require(NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: 1,
            pixelsHigh: 1,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        ))
        bitmap.setColor(.red, atX: 0, y: 0)
        let png = try #require(bitmap.representation(using: .png, properties: [:]))
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        pasteboard.clearContents()
        #expect(pasteboard.setData(png, forType: .png))

        let inserted = try #require(terminalPasteText(from: pasteboard))
        #expect(inserted.hasPrefix("'") && inserted.hasSuffix("'"))
        let path = String(inserted.dropFirst().dropLast())
        defer { try? FileManager.default.removeItem(atPath: path) }
        #expect(path.hasSuffix(".png"))
        let saved = try Data(contentsOf: URL(fileURLWithPath: path))
        let decoded = try #require(NSBitmapImageRep(data: saved))
        #expect(decoded.pixelsWide == 1 && decoded.pixelsHigh == 1)
    }

    @Test("raw image bytes take the provider shortcut; files and text do not")
    func imageShortcutRouting() throws {
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let png = try onePixelPNG()

        pasteboard.clearContents()
        #expect(pasteboard.setData(png, forType: .png))
        #expect(ghosttyPrefersImageShortcut(pasteboard))

        // A copied image *file* pastes as its path, matching drop behavior.
        let file = FileManager.default.temporaryDirectory
            .appendingPathComponent("shortcut-\(UUID().uuidString).png")
        try png.write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }
        pasteboard.clearContents()
        #expect(pasteboard.writeObjects([file as NSURL]))
        #expect(!ghosttyPrefersImageShortcut(pasteboard))

        pasteboard.clearContents()
        #expect(pasteboard.setString("plain text", forType: .string))
        #expect(!ghosttyPrefersImageShortcut(pasteboard))
    }

    @Test("a discarded pooled view is not revived by a later reopen")
    @MainActor
    func poolDiscardMintsFreshView() {
        let pool = GhosttySurfacePool()
        let view = pool.view(for: .session("x"))
        #expect(pool.view(for: .session("x")) === view)

        pool.discard(view)

        let reopened = pool.view(for: .session("x"))
        #expect(reopened !== view)
        // A delayed callback from the old view must not discard its replacement.
        pool.discard(view)
        #expect(pool.view(for: .session("x")) === reopened)
        #expect(!pool.hasSurface(.session("x")))
    }

    private func onePixelPNG() throws -> Data {
        let bitmap = try #require(NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: 1,
            pixelsHigh: 1,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        ))
        bitmap.setColor(.red, atX: 0, y: 0)
        return try #require(bitmap.representation(using: .png, properties: [:]))
    }
#endif

    @Test("file, image, and text drags are accepted; foreign drags are refused")
    func dropAcceptance() {
        #expect(ghosttyAcceptsDrop([.fileURL]))
        #expect(ghosttyAcceptsDrop([.png]))
        #expect(ghosttyAcceptsDrop([.tiff, .color]))
        #expect(ghosttyAcceptsDrop([.string]))
        #expect(!ghosttyAcceptsDrop([.color]))
        #expect(!ghosttyAcceptsDrop([]))
        #expect(!ghosttyAcceptsDrop(nil))
    }
}
#endif
