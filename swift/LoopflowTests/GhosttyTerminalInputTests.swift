#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
#if canImport(GhosttyKit)
    @Test("An off-window representable cannot steal a displayed terminal")
    @MainActor
    func offWindowTerminalMount() async throws {
        _ = NSApplication.shared
        let pool = GhosttySurfacePool()
        let identity = TerminalIdentity.shell("off-window-proof")
        let terminal = pool.view(for: identity)
        let content = GhosttyTerminalView(
            workingDirectory: NSTemporaryDirectory(), argv: ["/bin/cat"], env: [:],
            terminal: identity, surfacePool: pool, isFocused: true
        )
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 600, height: 400),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let live = NSHostingView(rootView: content)
        window.contentView = live
        defer { window.contentView = nil; pool.release(identity) }
        live.layoutSubtreeIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
        let surface = try #require(terminal.surface)
        try #require(terminal.window === window)
        #expect(window.makeFirstResponder(terminal))
        // SwiftUI may prepare a replacement subtree without ever displaying it.
        autoreleasepool {
            let speculative = NSHostingView(rootView: content)
            speculative.frame = live.frame
            speculative.layoutSubtreeIfNeeded()
            _ = speculative.fittingSize
            speculative.layoutSubtreeIfNeeded()
            #expect(terminal.window === window)
            #expect(terminal.isDescendant(of: live))
        }
        try await Task.sleep(for: .milliseconds(100))
        #expect(terminal.window === window)
        #expect(terminal.isDescendant(of: live))
        #expect(window.firstResponder === terminal)
        #expect(terminal.surface == surface)
    }

    @Test("Dismantling an old mount keeps the retained terminal in its new mount")
    @MainActor
    func retainedTerminalRemount() async throws {
        _ = NSApplication.shared
        let pool = GhosttySurfacePool()
        let identity = TerminalIdentity.shell("remount-proof")
        let terminal = pool.view(for: identity)
        let content = GhosttyTerminalView(
            workingDirectory: NSTemporaryDirectory(), argv: ["/bin/cat"], env: [:],
            terminal: identity, surfacePool: pool
        )
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 600, height: 400),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let root = NSView(frame: window.contentLayoutRect)
        window.contentView = root
        defer {
            window.contentView = nil
            pool.release(identity)
        }
        let oldMount = NSHostingView(rootView: AnyView(content.disabled(false)))
        oldMount.frame = root.bounds
        root.addSubview(oldMount)
        root.layoutSubtreeIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
        let surface = try #require(terminal.surface)
        #expect(terminal.window === window)

        // A split mounts its new subtree before SwiftUI retires the old subtree.
        let newMount = NSHostingView(rootView: AnyView(content))
        newMount.frame = root.bounds
        root.addSubview(newMount)
        root.layoutSubtreeIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
        try #require(terminal.isDescendant(of: newMount))
        #expect(window.makeFirstResponder(terminal))
        // The departing representable can still receive environment updates.
        // It no longer owns the terminal's focus or size after transfer.
        oldMount.rootView = AnyView(content.disabled(true))
        root.layoutSubtreeIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
        #expect(terminal.isDescendant(of: newMount))
        #expect(window.firstResponder === terminal)
        #expect(terminal.surface == surface)

        oldMount.rootView = AnyView(EmptyView())
        root.layoutSubtreeIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
        #expect(terminal.window === window)
        #expect(terminal.isDescendant(of: newMount))
        #expect(terminal.surface == surface)
    }

import GhosttyKit
#endif
@testable import LoopflowMac
@testable import Loopflow

#if canImport(GhosttyKit)
/// Shell block behavior that needs no display, so it runs headless.
@Suite("Embedded terminal shell blocks")
struct GhosttyShellBlockTests {
    @Test("the default-prompt shell gets a context header and reports a failed command")
    func zshBootstrapMarksHeaderAndFailure() throws {
        let resources = try #require(GhosttyRuntimeResources.directoryURL)
        let bootstrap = try #require(LoopflowZshBootstrap.install())
        let home = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-zsh-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: home, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: home) }

        func run(rc: String) throws -> String {
            try rc.write(to: home.appendingPathComponent(".zshrc"), atomically: true, encoding: .utf8)
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/bin/zsh")
            process.arguments = [
                "-i",
                "-c",
                """
                source ~/.zshrc
                _loopflow_prompt
                _ghostty_deferred_init
                print -Pn "$PS1"
                _ghostty_preexec "loopflow-missing-command"
                loopflow-missing-command
                _ghostty_precmd
                print -Pn "$PS1"
                """,
            ]
            process.currentDirectoryURL = home
            process.environment = [
                "HOME": home.path,
                "PATH": "/usr/bin:/bin",
                "GHOSTTY_RESOURCES_DIR": resources.path,
                "GHOSTTY_SHELL_FEATURES": "",
                "TERM": "xterm-256color",
                "ZDOTDIR": bootstrap,
            ]
            let stdout = Pipe()
            process.standardOutput = stdout
            process.standardError = FileHandle.nullDevice
            try process.run()
            // Drain before waiting: a full pipe would block the shell forever.
            let output = stdout.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            return String(decoding: output, as: UTF8.self)
        }

        func expectInOrder(_ markers: [String], in output: String) throws {
            var remaining = output[...]
            for marker in markers {
                let range = try #require(remaining.range(of: marker), "missing \(marker.debugDescription)")
                remaining = remaining[range.upperBound...]
            }
        }

        // The stock prompt becomes a blank row, a concealed header row holding
        // the directory, then the command line.
        let stock = try run(rc: "PS1='\(LoopflowZshBootstrap.macOSDefaultPrompt)'\n")
        try expectInOrder([
            "\u{1B}]133;A;cl=line\u{7}",
            "\n\u{1B}]133;A;k=s\u{7}",
            "\u{1B}[8m\(GhosttyBlockHeader.marker)",
            home.lastPathComponent,
            "\u{1B}[28m",
            "\n\u{1B}]133;A;k=s\u{7}",
            "\u{276F}",
            "\u{1B}]133;B\u{7}",
            "\u{1B}]133;C\u{7}",
            "\u{1B}]133;D;127\u{7}",
            "\u{1B}]133;A;cl=line\u{7}",
        ], in: stock)

        // A prompt someone chose is theirs, and still marks its commands.
        let custom = try run(rc: "PS1='mine> '\n")
        #expect(!custom.contains("\u{276F}"))
        try expectInOrder([
            "\u{1B}]133;A;cl=line\u{7}",
            "mine> ",
            "\u{1B}]133;B\u{7}",
            "\u{1B}]133;C\u{7}",
            "\u{1B}]133;D;127\u{7}",
        ], in: custom)
    }

    @Test("command blocks are full-width bands on the padded grid's rows")
    func commandBlockLayout() throws {
        let bounds = CGRect(x: 0, y: 0, width: 300, height: 200)
        let block = GhosttyCommandBlockLayout(startRow: 1, endRow: 3, exitCode: nil, selected: false)
        let frame = ghosttyCommandBlockFrame(block, bounds: bounds, cellHeight: 18)

        // Rows start below the top padding; spare height stays at the bottom.
        #expect(frame == CGRect(x: 0, y: 200 - 8 - 4 * 18, width: 300, height: 54))
        func row(atY y: CGFloat) -> Int? {
            ghosttyViewportRow(at: CGPoint(x: 150, y: y), bounds: bounds, rows: 10, cellHeight: 18)
        }
        #expect(row(atY: frame.maxY - 1) == 1)
        #expect(row(atY: frame.minY + 1) == 3)
        #expect(row(atY: 196) == nil)
        #expect(row(atY: 200 - 8 - 10 * 18 - 1) == nil)

        // Under the Loopflow prompt the blank row between two blocks is split:
        // each edge beside one sits half a row lower.
        let padded = ghosttyCommandBlockFrame(
            block, bounds: bounds, cellHeight: 18, blankRowAbove: true, blankRowBelow: true
        )
        #expect(padded == frame.offsetBy(dx: 0, dy: -9))
        let scrolledPastHeader = ghosttyCommandBlockFrame(
            block, bounds: bounds, cellHeight: 18, blankRowBelow: true
        )
        #expect(scrolledPastHeader.maxY == frame.maxY)
        #expect(scrolledPastHeader.minY == padded.minY)
    }

    @Test("a header is the marked prompt row, looked for only where a prompt can be")
    func blockHeader() throws {
        let marker = GhosttyBlockHeader.marker
        #expect(GhosttyBlockHeader.text(inRow: "\(marker)~/src/loopflow   ") == "~/src/loopflow")
        #expect(GhosttyBlockHeader.text(inRow: "~/src/loopflow") == nil)
        #expect(GhosttyBlockHeader.text(inRow: "\u{276F} ls") == nil)
        #expect(GhosttyBlockHeader.text(inRow: marker) == nil)

        // Rows 0-1: an empty prompt. Rows 2-6: a block (blank, header,
        // command, two output rows). Rows 7-9: the live prompt.
        let block = GhosttyCommandBlockLayout(startRow: 2, endRow: 6, exitCode: 0, selected: false)
        #expect(GhosttyBlockHeader.candidateRows(blocks: [block], rows: 10) == [0, 1, 2, 3, 7, 8, 9])

        // The header is drawn on its own row, inside the grid's padding.
        let frame = ghosttyBlockHeaderFrame(
            row: 3, bounds: CGRect(x: 0, y: 0, width: 300, height: 200), cellHeight: 18
        )
        #expect(frame == CGRect(x: 12, y: 200 - 8 - 4 * 18, width: 276, height: 18))
    }

    @Test("only a reported failure is red; a resting block has no fill")
    func commandBlockStyle() throws {
        func style(exit: Int?, selected: Bool = false, hovered: Bool = false) -> GhosttyCommandBlockStyle {
            GhosttyCommandBlockStyle(
                block: GhosttyCommandBlockLayout(startRow: 0, endRow: 1, exitCode: exit, selected: selected),
                hovered: hovered
            )
        }
        func isRed(_ color: NSColor) -> Bool {
            color.alphaComponent > 0 && color.redComponent > 0.8 && color.greenComponent < 0.4
        }

        #expect(style(exit: 0).fill.alphaComponent == 0)
        #expect(style(exit: nil).fill.alphaComponent == 0)
        #expect(isRed(style(exit: 127).fill))
        #expect(isRed(style(exit: 1, selected: true).fill))
        #expect(!isRed(style(exit: 0, selected: true).fill))
        #expect(!isRed(style(exit: nil, hovered: true).fill))
        // Selection is never red: a selected failure keeps its fill and changes its bar.
        #expect(isRed(style(exit: 1).accent))
        #expect(!isRed(style(exit: 1, selected: true).accent))
        #expect(!isRed(style(exit: 0, selected: true).accent))
        #expect(style(exit: 0, selected: true).accent == style(exit: 1, selected: true).accent)
    }

    @Test("dim text stays readable on every block fill")
    @MainActor
    func commandBlockFillContrast() throws {
        func rgb(_ hex: UInt) -> [CGFloat] {
            [CGFloat((hex >> 16) & 0xFF), CGFloat((hex >> 8) & 0xFF), CGFloat(hex & 0xFF)].map { $0 / 255 }
        }
        func luminance(_ rgb: [CGFloat]) -> CGFloat {
            let linear = rgb.map { $0 <= 0.03928 ? $0 / 12.92 : pow(($0 + 0.055) / 1.055, 2.4) }
            return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
        }
        // Palette 8 is the dimmest color a shell prints text in.
        let config = GhosttyManager.loopflowConfig
        let entry = try #require(config.range(of: "palette = 8=#"))
        let dim = rgb(try #require(UInt(config[entry.upperBound...].prefix(6), radix: 16)))
        let background = rgb(TerminalPalette.backgroundHex)

        for exit in [0, 1] {
            for (selected, hovered) in [(false, true), (true, false), (false, false)] {
                let fill = try #require(GhosttyCommandBlockStyle(
                    block: GhosttyCommandBlockLayout(startRow: 0, endRow: 1, exitCode: exit, selected: selected),
                    hovered: hovered
                ).fill.usingColorSpace(.sRGB))
                let alpha = fill.alphaComponent
                let shown = zip([fill.redComponent, fill.greenComponent, fill.blueComponent], background)
                    .map { $0 * alpha + $1 * (1 - alpha) }
                let ratio = (luminance(dim) + 0.05) / (luminance(shown) + 0.05)
                #expect(ratio >= 3, "exit \(exit) selected \(selected) hovered \(hovered): \(ratio)")
            }
        }
    }

    @Test("a right-click on the selected block keeps it; anywhere else goes to the terminal")
    func rightClickKeepsSelectedBlock() {
        let blocks = [
            GhosttyCommandBlockLayout(startRow: 0, endRow: 2, exitCode: 0, selected: false),
            GhosttyCommandBlockLayout(startRow: 3, endRow: 5, exitCode: 1, selected: true),
        ]
        #expect(ghosttyRightClickKeepsBlock(row: 3, blocks: blocks))
        #expect(ghosttyRightClickKeepsBlock(row: 5, blocks: blocks))
        // Another block, the live prompt and the padding are Ghostty's to handle.
        #expect(!ghosttyRightClickKeepsBlock(row: 1, blocks: blocks))
        #expect(!ghosttyRightClickKeepsBlock(row: 6, blocks: blocks))
        #expect(!ghosttyRightClickKeepsBlock(row: nil, blocks: blocks))
    }

    @Test("the embedded config parses, and Command-Up and Command-Down jump between prompts")
    @MainActor
    func embeddedConfigBindsPromptJumps() throws {
        try #require(GhosttyManager.libraryReady)
        let config = try #require(ghostty_config_new())
        defer { ghostty_config_free(config) }
        let path = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-ghostty-\(UUID().uuidString)")
        try GhosttyManager.embeddedConfig.write(to: path, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: path) }
        path.path.withCString { ghostty_config_load_file(config, $0) }
        ghostty_config_finalize(config)
        #expect(ghostty_config_diagnostics_count(config) == 0)

        for (action, key) in [("jump_to_prompt:-1", GHOSTTY_KEY_ARROW_UP), ("jump_to_prompt:1", GHOSTTY_KEY_ARROW_DOWN)] {
            let trigger = action.withCString { ghostty_config_trigger(config, $0, UInt(action.utf8.count)) }
            #expect(trigger.tag == GHOSTTY_TRIGGER_PHYSICAL)
            #expect(trigger.key.physical == key)
            #expect(trigger.mods == GHOSTTY_MODS_SUPER)
        }
    }
}
#endif

@Suite("Embedded terminal input", .requiresDisplay)
struct GhosttyTerminalInputTests {
    // SwiftPM links GhosttyKit; the Xcode compile-check target builds the fallback.
#if canImport(GhosttyKit)
    @Test("Shell-launched sessions focus their live terminal and external clients stay elsewhere")
    @MainActor
    func shellSessionAttachment() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let registry = SessionsWorkspaceRegistry(localHomeId: fixtureHomeId)
        let workspace = registry.workspace(for: fixtureWorkspace(NSTemporaryDirectory()))
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
                "id": id, "run_id": id, "interactive": true, "work": NSNull(), "title": id,
                "detail": "test", "cwd": NSTemporaryDirectory(), "state": "active",
                "ready_summary": NSNull(), "work_path": NSNull(), "actions": sessionActionFixture(state: "active"), "title_source": "generated", "task_primary": false, "flow_membership": ["kind": "independent"], "task_ids": [], "terminal_ids": [pane], "open_argv": ["unused"],
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

    @Test("text and block selection replace each other, and copy follows the one shown")
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
        // Feed known OSC 133 boundaries through a real PTY and Ghostty parser:
        // a command that succeeded, one that failed, and the live prompt.
        view.command = #"/bin/sh -c 'printf "\033]133;A\007$ \033]133;B\007echo alpha\r\n\033]133;C\007alpha\r\n\033]133;D;0\007\033]133;A\007$ \033]133;B\007sdl\r\n\033]133;C\007command not found: sdl\r\n\033]133;D;127\007\033]133;A\007$ \033]133;B\007ready"; exec /bin/cat'"#
        view.createSurface(manager: manager)
        defer { view.handleSurfaceClose() }
        let surface = try #require(view.surface)

        func blocks() -> [ghostty_command_block_s] {
            var raw = Array(repeating: ghostty_command_block_s(), count: 10)
            let count = raw.withUnsafeMutableBufferPointer {
                ghostty_surface_command_blocks(surface, $0.baseAddress, $0.count)
            }
            return Array(raw.prefix(count))
        }
        let deadline = ContinuousClock.now + .seconds(3)
        // Read once more after a delayed wake-up before declaring timeout.
        while blocks().count != 2, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        let initial = blocks()
        try #require(initial.count == 2)
        #expect(initial.map(\.exit_code) == [0, 127])
        #expect(initial.allSatisfy { !$0.selected })
        view.setFrameSize(view.frame.size)

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

        func point(row: UInt16, column: CGFloat) -> CGPoint {
            let size = ghostty_surface_size(surface)
            let scale = window.backingScaleFactor
            return CGPoint(
                x: GhosttyTerminalPadding.x + (column + 0.5) * CGFloat(size.cell_width_px) / scale,
                y: view.bounds.height - GhosttyTerminalPadding.y
                    - (CGFloat(row) + 0.5) * CGFloat(size.cell_height_px) / scale
            )
        }
        func mouse(_ type: NSEvent.EventType, _ point: CGPoint) throws {
            let event = try #require(NSEvent.mouseEvent(
                with: type, location: view.convert(point, to: nil), modifierFlags: [],
                timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber, context: nil,
                eventNumber: 1, clickCount: 1, pressure: 1
            ))
            switch type {
            case .leftMouseDown: view.mouseDown(with: event)
            case .leftMouseDragged: view.mouseDragged(with: event)
            default: view.mouseUp(with: event)
            }
        }
        func click(row: UInt16) throws {
            try mouse(.leftMouseDown, point(row: row, column: 20))
            try mouse(.leftMouseUp, point(row: row, column: 20))
        }
        func copied() -> String? {
            pasteboard.clearContents()
            _ = view.performKeyEquivalent(with: copy)
            return pasteboard.string(forType: .string)
        }

        // A click anywhere in a block selects it; another block replaces it.
        try click(row: initial[0].start_row)
        #expect(blocks().map(\.selected) == [true, false])
        #expect(copied() == "echo alpha\nalpha")
        try click(row: initial[1].end_row)
        #expect(blocks().map(\.selected) == [false, true])
        #expect(copied() == "sdl\ncommand not found: sdl")

        // Dragging across text inside a block selects text and ends the block.
        let outputRow = initial[1].end_row
        try mouse(.leftMouseDown, point(row: outputRow, column: 0))
        try mouse(.leftMouseDragged, point(row: outputRow, column: 6))
        try mouse(.leftMouseUp, point(row: outputRow, column: 6))
        #expect(ghostty_surface_has_selection(surface))
        #expect(blocks().allSatisfy { !$0.selected })
        #expect(copied() == "command")

        // Clicking a block ends the text selection.
        try click(row: initial[0].end_row)
        #expect(!ghostty_surface_has_selection(surface))
        #expect(blocks().map(\.selected) == [true, false])

        // Command-C left the block selected; a key that goes to the shell ends
        // it, and so does Escape.
        func press(_ characters: String, keyCode: UInt16) throws {
            view.keyDown(with: try #require(NSEvent.keyEvent(
                with: .keyDown, location: .zero, modifierFlags: [],
                timestamp: 0, windowNumber: window.windowNumber, context: nil,
                characters: characters, charactersIgnoringModifiers: characters,
                isARepeat: false, keyCode: keyCode
            )))
        }
        #expect(copied() == "echo alpha\nalpha")
        #expect(blocks().map(\.selected) == [true, false])
        try press("x", keyCode: 7)
        #expect(blocks().allSatisfy { !$0.selected })
        try click(row: initial[0].end_row)
        try press("\u{1B}", keyCode: 53)
        #expect(blocks().allSatisfy { !$0.selected })
        try click(row: initial[0].end_row)
        #expect(blocks().map(\.selected) == [true, false])

        // The selection and the failure survive a resize that wraps output.
        view.setFrameSize(CGSize(width: 150, height: 300))
        let narrowDeadline = ContinuousClock.now + .seconds(3)
        while blocks().last.map({ $0.end_row - $0.start_row }) == 1, ContinuousClock.now < narrowDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        let wrapped = blocks()
        #expect(wrapped.map(\.selected) == [true, false])
        #expect(wrapped.map(\.exit_code) == [0, 127])
        #expect(copied() == "echo alpha\nalpha")

        // The live prompt is not a block: clicking it leaves nothing selected.
        try click(row: wrapped[1].end_row + 1)
        #expect(blocks().allSatisfy { !$0.selected })
        #expect(copied() == nil)
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
