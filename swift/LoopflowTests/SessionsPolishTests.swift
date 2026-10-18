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
        let first = fixtureWork(registry, fixtureWorkspace("/tmp/repo"))
        let again = fixtureWork(registry, fixtureWorkspace("/tmp/repo"))
        let other = fixtureWork(registry, fixtureWorkspace("/tmp/other"))
        #expect(first === again)
        #expect(first !== other)
        #expect(first.multiplexer === again.multiplexer)
    }

    @Test("Separate windows never share a multiplexer or surface pool")
    func separateWindowsGetSeparateWorkspaces() {
        let windowA = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let windowB = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let a = fixtureWork(windowA, fixtureWorkspace("/tmp/repo"))
        let b = fixtureWork(windowB, fixtureWorkspace("/tmp/repo"))
        #expect(a !== b)
        #expect(a.multiplexer !== b.multiplexer)
        #expect(a.surfaces !== b.surfaces)
    }

    @Test("A shell exit from another Machine cannot close a same-ID pane")
    func shellExitKeepsMachineIdentity() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let local = fixtureWork(registry, fixtureWorkspace("/same/path"))
        local.multiplexer.newShell(location: local.location)
        let shell = local.multiplexer.focusedPane
        let tab = try #require(shell.content.selectedShell)
        #expect(registry.key(containingShell: tab.id, machineId: "peer") == nil)
        NotificationCenter.default.post(name: .ghosttySurfaceClosed,
            object: TerminalIdentity.shell(tab.id, machineId: "peer"))
        #expect(local.multiplexer.focusedPane == shell)
    }

    @Test("A shell exit closes its retained pane while SessionsView is absent")
    func shellExitSurvivesNavigation() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let workspace = fixtureWork(registry, fixtureWorkspace("/tmp/repo"))
        workspace.multiplexer.newShell(location: workspace.location)
        let shell = workspace.multiplexer.focusedPane
        let tab = try #require(shell.content.selectedShell)
        let other = fixtureWork(registry, fixtureWorkspace("/tmp/other"))
        other.multiplexer.newShell(location: other.location)
        let otherShell = other.multiplexer.focusedPane

        NotificationCenter.default.post(
            name: .ghosttySurfaceClosed, object: TerminalIdentity.session(tab.id, machineId: fixtureMachineId)
        )
        #expect(workspace.multiplexer.focusedPane == shell)
        NotificationCenter.default.post(
            name: .ghosttySurfaceClosed, object: TerminalIdentity.shell(tab.id, machineId: fixtureMachineId)
        )

        #expect(fixtureWork(registry, fixtureWorkspace("/tmp/repo")).multiplexer.focusedPane.content == .empty)
        #expect(other.multiplexer.focusedPane == otherShell)
    }
}
#endif
