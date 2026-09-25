#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Sessions polish")
@MainActor
struct SessionsPolishTests {
    @Test("One window's registry retains a repo workspace across visits")
    func registryRetainsWorkspacePerRepo() {
        let registry = SessionsWorkspaceRegistry()
        let first = registry.workspace(for: "/tmp/repo")
        let again = registry.workspace(for: "/tmp/repo")
        let other = registry.workspace(for: "/tmp/other")
        #expect(first === again)
        #expect(first !== other)
        #expect(first.multiplexer === again.multiplexer)
    }

    @Test("Separate windows never share a multiplexer or surface pool")
    func separateWindowsGetSeparateWorkspaces() {
        let windowA = SessionsWorkspaceRegistry()
        let windowB = SessionsWorkspaceRegistry()
        let a = windowA.workspace(for: "/tmp/repo")
        let b = windowB.workspace(for: "/tmp/repo")
        #expect(a !== b)
        #expect(a.multiplexer !== b.multiplexer)
        #expect(a.surfaces !== b.surfaces)
    }

    @Test("A shell exit closes its retained pane while SessionsView is absent")
    func shellExitSurvivesNavigation() {
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: "/tmp/repo")
        workspace.multiplexer.newShell()
        let paneId = workspace.multiplexer.focusedPaneId
        let other = registry.workspace(for: "/tmp/other")
        other.multiplexer.newShell()

        NotificationCenter.default.post(
            name: .ghosttySurfaceClosed, object: TerminalIdentity.session(paneId)
        )
        #expect(workspace.multiplexer.focusedPane.content == .shell)
        NotificationCenter.default.post(
            name: .ghosttySurfaceClosed, object: TerminalIdentity.shell(paneId)
        )

        #expect(registry.workspace(for: "/tmp/repo").multiplexer.focusedPane.content == .empty)
        #expect(other.multiplexer.focusedPane.content == .shell)
    }

    private func record(kind: String, state: String = "active") throws -> SessionRecord {
        try JSONDecoder().decode(
            SessionRecord.self,
            from: Data(
                """
                {
                  "id": "polish", "run_id": "polish",
                  "kind": "\(kind)",
                  "work": null,
                  "title": "Polish fixture",
                  "detail": "fixture-provider",
                  "cwd": "/tmp",
                  "state": "\(state)",
                  "ready_summary": null,
                  "work_path": "product / Desktop / LOO-291",
                  "actions": \(sessionActionFixtureJSON(kind: kind, state: state)),
                  "title_source": "generated", "flow_membership": {"kind": "independent"}, "terminal_ids": [],
                  "open_argv": ["lf", "session", "open", "polish"]
                }
                """.utf8
            )
        )
    }
}
#endif
