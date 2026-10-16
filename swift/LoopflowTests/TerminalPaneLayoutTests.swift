#if os(macOS)
import AppKit
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Terminal pane clearance")
@MainActor
struct TerminalPaneLayoutTests {
    @Test("Nested corner centers coincide and split gaps match the outer inset",
          arguments: [SplitAxis.vertical, .horizontal], [false, true])
    func cornerClearance(axis: SplitAxis, dark: Bool) throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("No commands during inspection") }
        let model = WorkModel(query: query, repoPath: "/repo")
        model.confirmMachine(fixtureMachineId)
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let theme = dark ? ThemeDefinition.dark : .light
        let view = SessionsContentView(model: model, repoPath: "/repo", workspaces: registry,
                                      machineId: fixtureMachineId, repository: "repo", query: query)
            .environment(\.theme, theme).environment(\.palette, theme.palette)
        let workspace = try #require(registry.selectedWork.flatMap { registry.retained($0) })
        workspace.presentation.content = .terminals
        let first = try #require(workspace.multiplexer.newShell(location: fixtureWorkspace("/repo")))
        _ = try #require(workspace.multiplexer.split(first.id, axis: axis, location: fixtureWorkspace("/repo")))
        // Prepared Sessions expose the production pane chrome without mounting
        // Ghostty or reading unmounted Observation environment objects.
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/session.json")
        var json = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: fixture)) as? [String: Any])
        json["task_primary"] = true
        let sessions = workspace.sessionStore(repoPath: "/repo", query: query)
        for pane in workspace.multiplexer.layout.allPanes {
            json["id"] = pane.id
            let record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: json))
            sessions.reconcile([record])
            sessions.adoptPreparation(record)
            workspace.multiplexer.replace(pane.id, with: .session(id: record.id, location: record.workspace?.identity))
        }
        registry.open(WorkWorkspaceKey(repository: "repo", kind: .wave, id: "companion"),
                      title: "Companion", location: fixtureWorkspace("/repo"), besideFocused: true)
        let inspection = try view.inspect()
        let layout = try inspection.find(viewWithAccessibilityIdentifier: "work-layout")
        #expect(try layout.padding(.bottom) == 10)
        #expect(try layout.padding(.leading) == layout.padding(.bottom))
        #expect(try layout.padding(.trailing) == layout.padding(.bottom))
        let outer = try inspection.find(viewWithAccessibilityIdentifier: "work-workspace-\(workspace.key.id)")
        let radius = try outer.clipShape(RoundedRectangle.self).cornerSize.width
        let multiplexer = try inspection.find(viewWithAccessibilityIdentifier: "sessions-multiplexer")
        let gap = try multiplexer.padding(.bottom)
        #expect(try multiplexer.padding(.trailing) == gap)
        #expect(radius - gap == PaneGeometry.cornerRadius)
        let outerStroke = try outer.overlay().find(ViewType.Shape.self)
        #expect(try outerStroke.strokeStyle().lineWidth == 1)
        let inactive = try inspection.find(viewWithAccessibilityIdentifier: "work-workspace-companion")
        let inactiveStroke = try inactive.overlay().find(ViewType.Shape.self)
        #expect(try inactiveStroke.strokeStyle().lineWidth == 1)
        let panes = outer.findAll(where: { (try? $0.accessibilityIdentifier())?.hasPrefix("session-pane-") == true })
        #expect(panes.count == 2)
        let bounds = CGRect(x: 0, y: 0, width: 500, height: 400)
        #expect(try inactiveStroke.path(in: bounds) == outerStroke.path(in: bounds))
        let outerPath = try outerStroke.path(in: bounds).strokedPath(outerStroke.strokeStyle())
        for pane in panes {
            let innerStroke = try pane.overlay().find(ViewType.Shape.self)
            #expect(try innerStroke.strokeStyle().lineWidth == 1)
            let innerPath = try innerStroke.path(in: bounds.insetBy(dx: gap, dy: gap)).strokedPath(innerStroke.strokeStyle())
            // Sample the actual stroked SwiftUI paths at both lower corners,
            // including their tangent points. Five points must remain clear.
            for right in [false, true] {
                let center = CGPoint(x: right ? bounds.maxX - radius : radius, y: bounds.maxY - radius)
                for degrees in stride(from: 0, through: 90, by: 5) {
                    let angle = Double(degrees) * .pi / 180
                    let dx = cos(angle) * (right ? 1.0 : -1.0)
                    let dy = sin(angle)
                    func point(_ distance: Double) -> CGPoint {
                        CGPoint(x: center.x + distance * dx, y: center.y + distance * dy)
                    }
                    let samples = Array(stride(from: 0.0, through: 16.0, by: 0.01))
                    let innerEdge = try #require(samples.last { innerPath.contains(point($0)) })
                    let outerEdge = try #require(samples.first { outerPath.contains(point($0)) })
                    // Path containment flattens curves; allow subpixel error.
                    #expect(abs(outerEdge - innerEdge - 5) < 0.15)
                }
            }
        }
        for extent: CGFloat in [300, 800] {
            let sizing = SplitSizing(extent: extent, firstMinimum: 240, secondMinimum: 240)
            #expect(sizing.divider == gap)
        }
    }
}

#if canImport(GhosttyKit)
import GhosttyKit

@Suite("Terminal pane frames", .requiresDisplay, .serialized)
@MainActor
struct TerminalPaneFrameTests {
    @Test("Native surfaces and columns fit inside split pane borders in both themes",
          arguments: [SplitAxis.vertical, .horizontal], [false, true])
    func splitClearance(axis: SplitAxis, dark: Bool) async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        try #require(manager.state == .ready)
        let previousTheme = manager.theme
        let theme = dark ? ThemeDefinition.dark : .light
        try manager.applyTheme(theme)
        defer { try? manager.applyTheme(previousTheme) }
        let query = RegistryQuery { args, _ in
            throw RegistryQueryError("Layout proof must not launch commands: \(args)")
        }
        let path = NSTemporaryDirectory()
        let model = WorkModel(query: query, repoPath: path)
        model.confirmMachine(fixtureMachineId)
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let view = SessionsContentView(model: model, repoPath: path, workspaces: registry,
                                      machineId: fixtureMachineId, repository: "repo", query: query)
            .environment(\.theme, theme)
            .environment(\.palette, theme.palette)
            .environment(\.colorScheme, theme.colorScheme)
        let workspace = try #require(registry.selectedWork.flatMap { registry.retained($0) })
        workspace.presentation.content = .terminals
        let store = workspace.multiplexer
        let location = fixtureWorkspace(path)
        let first = try #require(store.newShell(command: ["/bin/cat"], location: location))
        let second = try #require(store.split(first.id, axis: axis, command: ["/bin/cat"], location: location))
        let terminals = try [first, second].map { registry.surfaces.view(for: try #require($0.terminal)) }
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1100, height: 800),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let host = NSHostingView(rootView: view)
        window.contentView = host
        defer {
            window.contentView = nil
            for terminal in terminals { registry.surfaces.release(terminal.terminal) }
        }
        for focused in [first.id, second.id] {
            store.setFocusedPane(focused)
            host.layoutSubtreeIfNeeded()
            try await Task.sleep(for: .milliseconds(100))
            let frames = terminals.map { $0.convert($0.bounds, to: host) }
            for (terminal, frame) in zip(terminals, frames) {
                #expect(terminal.window === window)
                // Even the surface itself, before Ghostty's internal padding,
                // clears the left/right rounded edge of its enclosing pane.
                let windowClearance = PaneGeometry.windowInset + PaneGeometry.spacing + PaneGeometry.cornerRadius
                #expect(frame.minX >= windowClearance - 1)
                #expect(host.bounds.width - frame.maxX >= windowClearance - 1)
                let bottomClearance = host.isFlipped ? host.bounds.height - frame.maxY : frame.minY
                #expect(bottomClearance >= windowClearance - 1)
                let surface = try #require(terminal.surface)
                let grid = ghostty_surface_size(surface)
                let pixels = terminal.convertToBacking(terminal.bounds).width
                #expect(abs(Double(grid.width_px) - pixels) <= 1)
                #expect(Double(grid.columns) * Double(grid.cell_width_px) < pixels)
            }
            let gap = axis == .vertical
                ? max(frames[0].minX, frames[1].minX) - min(frames[0].maxX, frames[1].maxX)
                : max(frames[0].minY, frames[1].minY) - min(frames[0].maxY, frames[1].maxY)
            #expect(gap >= PaneGeometry.spacing + 2 * PaneGeometry.cornerRadius - 1)
        }
    }
}
#endif
#endif
