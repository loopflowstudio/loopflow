import Foundation
import Loopflow
import UIKit
import XCTest
@testable import LoopflowiOS

/// Opt-in acceptance against an isolated Machine registry through host
/// Tailscale Serve. The runner transfers a one-use ticket directly to Documents;
/// neither the test nor its assertions print ticket or credential contents.
@MainActor
final class MachineConnectionIntegrationTests: XCTestCase {
    private struct SessionProof: Decodable { let session: String }
    private var window: UIWindow?

    func testPairAndReadThroughHostTailscale() async throws {
        let setup = URL.documentsDirectory.appending(path: "LoopflowIntegration/setup.json")
        guard FileManager.default.fileExists(atPath: setup.path) else {
            throw XCTSkip("No isolated Machine setup ticket supplied")
        }
        let data = try Data(contentsOf: setup)
        try FileManager.default.removeItem(at: setup)
        let ticket = try MachinePairingTicket.decode(data)
        let proofURL = setup.deletingLastPathComponent().appending(path: "session.json")
        let proof: SessionProof?
        if FileManager.default.fileExists(atPath: proofURL.path) {
            proof = try JSONDecoder().decode(SessionProof.self, from: Data(contentsOf: proofURL))
            try FileManager.default.removeItem(at: proofURL)
        } else { proof = nil }
        let previous = try MachineKeychain.load()
        defer {
            do {
                if let previous { try MachineKeychain.save(previous) }
                else { try MachineKeychain.forget() }
            } catch { XCTFail("Could not restore the simulator's previous Machine credential") }
        }
        let credential = try await MachinePairing.redeem(ticket)
        XCTAssertNil(credential.machineID)
        let client = RemoteMobileClient(credential: try XCTUnwrap(MachineKeychain.load()))
        let tasks = try await client.workingTasks()
        if proof == nil { XCTAssertTrue(tasks.isEmpty, "An empty registry is required without a Session fixture") }
        let confirmed = try XCTUnwrap(MachineKeychain.load())
        XCTAssertNotNil(confirmed.machineID)
        XCTAssertTrue(confirmed.token == credential.token, "Bootstrap must preserve the issued grant")
        let reopened = RemoteMobileClient(credential: confirmed)
        let refreshed = try await reopened.workingTasks()
        XCTAssertEqual(refreshed.map(\.work), tasks.map(\.work))
        if let proof { try await exerciseSession(proof, client: reopened, tasks: refreshed) }
    }

    private func exerciseSession(_ proof: SessionProof, client: RemoteMobileClient,
                                 tasks: [WorkViewPlan.Content]) async throws {
        XCTAssertEqual(tasks.count, 1, "Only the isolated fixture Task may be present")
        let task = try XCTUnwrap(tasks.first { $0.primarySession == proof.session })
        let workspace = MobileWorkspace(client: client)
        let controller = SessionTerminalController(workspace: workspace)
        controller.loadViewIfNeeded()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 700))
        window.rootViewController = controller
        window.isHidden = false
        self.window = window
        defer { workspace.leave(); controller.terminal.updateUiClosed(); self.window = nil }
        controller.view.frame = window.bounds
        controller.view.layoutIfNeeded()

        func screen() -> String { controller.terminal.accessibilityPageContent() ?? "" }
        await workspace.open(task)
        try await wait("Initial draft did not reach the native terminal") { screen().contains("desktop draft") }
        XCTAssertEqual(workspace.state, .viewing)
        workspace.send(Data("must not queue".utf8))
        workspace.requestControl()
        try await wait("Phone did not receive keyboard control") { workspace.state == .controlling }
        controller.update(theme: .light)
        controller.terminal.insertText(" + phone")
        try await wait("Phone editing did not reach the owner") { screen().contains("desktop draft + phone") }

        workspace.suspend()
        controller.update(theme: .light)
        // A newer viewer takes control while the phone is disconnected. Use the
        // same carrier/protocol as Desktop, with Desktop-sized geometry.
        let desktop = try await client.attach(sessionID: proof.session)
        defer { desktop.close() }
        var desktopInput = false
        let desktopEvents = Task {
            for await event in desktop.events {
                if case .control(let input, _) = event { desktopInput = input }
            }
        }
        defer { desktopEvents.cancel() }
        try desktop.requestControl(size: TerminalGeometry(columns: 120, rows: 40))
        try await wait("Second viewer did not receive control") { desktopInput }

        await workspace.reconnect()
        try await wait("Reconnection did not recover the unfinished draft") {
            workspace.state == .viewing && screen().contains("desktop draft + phone")
        }
        controller.update(theme: .light)
        XCTAssertEqual(controller.terminal.getTerminal().cols, 120)
        XCTAssertEqual(controller.terminal.getTerminal().rows, 40)
        XCTAssertTrue(desktopInput, "A returning phone must not take newer control")
        try desktop.send(Data(" + desktop".utf8))
        try await wait("The newer viewer could not continue editing") {
            screen().contains("desktop draft + phone + desktop")
        }
        workspace.requestControl()
        try await wait("Explicit handoff failed") { workspace.state == .controlling && !desktopInput }
        controller.update(theme: .light)
        controller.terminal.insertText("\n")
        try await wait("The existing provider did not finish") { workspace.state == .ended }
        XCTAssertEqual(workspace.selected?.primarySession, proof.session)
        XCTAssertTrue(screen().contains("accepted:"))
    }

    private func wait(_ reason: String, until predicate: () -> Bool) async throws {
        let deadline = Date().addingTimeInterval(10)
        while !predicate() {
            guard Date() < deadline else { throw RegistryQueryError(reason) }
            try await Task.sleep(for: .milliseconds(20))
        }
    }
}
