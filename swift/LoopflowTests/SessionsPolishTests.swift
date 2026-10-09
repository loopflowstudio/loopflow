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
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let first = registry.workspace(for: fixtureWorkspace("/tmp/repo"))
        let again = registry.workspace(for: fixtureWorkspace("/tmp/repo"))
        let other = registry.workspace(for: fixtureWorkspace("/tmp/other"))
        #expect(first === again)
        #expect(first !== other)
        #expect(first.multiplexer === again.multiplexer)
    }

    @Test("Separate windows never share a multiplexer or surface pool")
    func separateWindowsGetSeparateWorkspaces() {
        let windowA = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let windowB = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let a = windowA.workspace(for: fixtureWorkspace("/tmp/repo"))
        let b = windowB.workspace(for: fixtureWorkspace("/tmp/repo"))
        #expect(a !== b)
        #expect(a.multiplexer !== b.multiplexer)
        #expect(a.surfaces !== b.surfaces)
    }

    @Test("A shell exit from another Machine cannot close a same-ID pane")
    func shellExitKeepsMachineIdentity() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let local = registry.workspace(for: fixtureWorkspace("/same/path"))
        local.multiplexer.newShell()
        let shell = local.multiplexer.focusedPane
        #expect(registry.path(containingShell: shell.id, machineId: "peer") == nil)
        NotificationCenter.default.post(name: .ghosttySurfaceClosed,
            object: TerminalIdentity.shell(shell.id, machineId: "peer"))
        #expect(local.multiplexer.focusedPane == shell)
    }

    @Test("A shell exit closes its retained pane while SessionsView is absent")
    func shellExitSurvivesNavigation() {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let workspace = registry.workspace(for: fixtureWorkspace("/tmp/repo"))
        workspace.multiplexer.newShell()
        let paneId = workspace.multiplexer.focusedPaneId
        let other = registry.workspace(for: fixtureWorkspace("/tmp/other"))
        other.multiplexer.newShell()

        NotificationCenter.default.post(
            name: .ghosttySurfaceClosed, object: TerminalIdentity.session(paneId, machineId: fixtureMachineId)
        )
        #expect(workspace.multiplexer.focusedPane.content == .shell)
        NotificationCenter.default.post(
            name: .ghosttySurfaceClosed, object: TerminalIdentity.shell(paneId, machineId: fixtureMachineId)
        )

        #expect(registry.workspace(for: fixtureWorkspace("/tmp/repo")).multiplexer.focusedPane.content == .empty)
        #expect(other.multiplexer.focusedPane.content == .shell)
    }
}
#endif
