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
        // Only a Task has panes to nest: its session beside the shell.
        let location = WorkspaceIdentity(machineId: fixtureMachineId, worktree: "/repo")
        let workspace = registry.workspace(for: .init(repository: "repo", kind: .task, id: "task-a"),
            title: "Task A", location: location)
        model.select(workspace.key.reference)
        workspace.presentation.content = .terminals
        workspace.multiplexer.load(sessionId: "first")
        let first = workspace.multiplexer.focusedPane
        let shell = try #require(workspace.multiplexer.newShell(focus: false))
        workspace.multiplexer.move(shell.id, beside: first.id, axis: axis)
        // Prepared Sessions expose the production pane chrome without mounting
        // Ghostty or reading unmounted Observation environment objects.
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/session.json")
        var json = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: fixture)) as? [String: Any])
        json["task_primary"] = true
        let sessions = workspace.sessionStore(repoPath: "/repo", query: query)
        json["id"] = first.id
        let record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: json))
        sessions.reconcile([record])
        sessions.adoptPreparation(record)
        workspace.multiplexer.replace(first.id, with: .session(id: record.id, location: record.workspace?.identity))
        let inspection = try view.inspect()
        let layout = try inspection.find(viewWithAccessibilityIdentifier: "work-layout")
        #expect(try layout.padding(.bottom) == 10)
        #expect(try layout.padding(.leading) == layout.padding(.bottom))
        #expect(try layout.padding(.trailing) == layout.padding(.bottom))
        let outer = try inspection.find(viewWithAccessibilityIdentifier: "work-workspace-\(workspace.key.id)")
        let radius = try outer.clipShape(UnevenRoundedRectangle.self).cornerRadii.bottomLeading
        let multiplexer = try inspection.find(viewWithAccessibilityIdentifier: "sessions-multiplexer")
        let gap = try multiplexer.padding(.bottom)
        #expect(try multiplexer.padding(.trailing) == gap)
        #expect(radius - gap == PaneGeometry.cornerRadius)
        let outerStroke = try outer.overlay().find(ViewType.Shape.self)
        #expect(try outerStroke.strokeStyle().lineWidth == 1)
        let panes = outer.findAll(where: { (try? $0.accessibilityIdentifier())?.hasPrefix("session-pane-") == true })
        #expect(panes.count == 2)
        let bounds = CGRect(x: 0, y: 0, width: 500, height: 400)
        let outerPath = try outerStroke.path(in: bounds).strokedPath(outerStroke.strokeStyle())
        for pane in panes {
            let innerStroke = try pane.overlay(0).find(ViewType.Shape.self)
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

#endif
