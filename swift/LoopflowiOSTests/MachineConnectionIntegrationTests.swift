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
    private struct SessionProof: Decodable {
        let session: String
        let creationRepository: String?
        let outputSession: String?
    }
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
        let observation = try await reopened.observeTasks()
        defer { observation.close() }
        var readings = observation.readings.makeAsyncIterator()
        let initial = try await readings.next()
        XCTAssertEqual(try XCTUnwrap(initial).get().map(\.work), tasks.map(\.work))
        try await observation.refresh()
        let replacement = try await readings.next()
        XCTAssertEqual(try XCTUnwrap(replacement).get().map(\.work), tasks.map(\.work))
        observation.close()
        if let proof { try await exerciseSession(proof, client: reopened, tasks: refreshed) }
        if let directory = proof?.creationRepository { try await exerciseCreation(client: reopened, directory: directory) }
        if let session = proof?.outputSession { try await exerciseOutput(client: reopened, session: session) }
    }

    private func exerciseOutput(client: RemoteMobileClient, session: String) async throws {
        let stalled = try await client.attach(sessionID: session)
        defer { stalled.close() }
        let active = try await client.attach(sessionID: session)
        defer { active.close() }
        let expected = 512 * 65536
        var received = 0
        var acknowledged = 0
        var controlled = false
        var complete = false
        var tail = ""
        var ended = false
        var problem: String?
        let reader = Task {
            for await event in active.events {
                switch event {
                case .control(let held, _): controlled = held
                case .output(let bytes):
                    received += bytes.filter { $0 == 120 }.count
                    while acknowledged < received / 65536 {
                        do { try active.send(Data("next\n".utf8)); acknowledged += 1 }
                        catch { problem = error.localizedDescription; return }
                    }
                    tail = String((tail + String(decoding: bytes.suffix(128), as: UTF8.self)).suffix(128))
                    if tail.contains("output-complete") { complete = true }
                case .ended: ended = true
                case .disconnected(let reason): problem = reason
                default: break
                }
            }
        }
        defer { reader.cancel() }
        try active.requestControl(size: TerminalGeometry(columns: 80, rows: 24))
        try await wait("Output fixture did not grant control") { controlled || problem != nil }
        XCTAssertNil(problem)
        try active.send(Data("start\n".utf8))
        try await wait("Sustained output did not complete", seconds: 60) { complete || problem != nil }
        XCTAssertNil(problem)
        XCTAssertEqual(received, expected, "The active phone must receive every output byte")

        var staleBytes = 0
        var stalledFinished = false
        let draining = Task {
            for await event in stalled.events {
                if case .output(let bytes) = event { staleBytes += bytes.count }
                if case .disconnected = event { stalledFinished = true; return }
            }
            stalledFinished = true
        }
        defer { draining.cancel() }
        try await wait("Stalled phone buffered the entire output instead of disconnecting", seconds: 15) { stalledFinished }
        XCTAssertLessThan(staleBytes, expected / 2, "A stopped reader must not accumulate the entire stream")
        print("Mobile output proof: active \(received) bytes, stalled \(staleBytes) bytes")
        let returning = try await client.attach(sessionID: session)
        defer { returning.close() }
        var freshScreen = false
        var passive = false
        let returningReader = Task {
            for await event in returning.events {
                if case .snapshot(let bytes) = event {
                    freshScreen = String(decoding: bytes, as: UTF8.self).contains("output-complete")
                }
                if case .control(let input, let geometry) = event {
                    XCTAssertFalse(input)
                    XCTAssertFalse(geometry)
                    passive = true
                }
            }
        }
        defer { returningReader.cancel() }
        try await wait("Stalled phone did not recover the current screen passively") { freshScreen && passive }
        try active.send(Data("done\n".utf8))
        try await wait("Output fixture did not end") { ended || problem != nil }
        XCTAssertTrue(ended)
    }

    private func exerciseCreation(client: RemoteMobileClient, directory: String) async throws {
        let store = MobileTaskDraftStore(url: FileManager.default.temporaryDirectory
            .appending(path: UUID().uuidString).appending(path: "creation.json"))
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let creation = MobileTaskCreation(client: client, store: store)
        await creation.prepare()
        XCTAssertNotNil(creation.context, creation.error ?? "Missing destinations")
        creation.selectRepository(directory)
        creation.editText("Create a Task from the native phone")
        XCTAssertTrue(creation.canCreate)
        await creation.submit()
        // A slow build/Graph may outlast one carrier deadline. Recover the same
        // admission; never create another request just to make the proof pass.
        if !creation.settled { await creation.submit() }
        let outcome = try XCTUnwrap(creation.draft.outcome, creation.error ?? "No creation result")
        XCTAssertEqual(outcome.state, .started, outcome.message ?? "Terminal did not start")
        let submission = try XCTUnwrap(creation.draft.submission)
        XCTAssertTrue(creation.saveForLater())
        creation.editText("A separate idea that must not be submitted")
        XCTAssertTrue(creation.saveDraft())
        let restored = MobileTaskCreation(client: client, store: store)
        XCTAssertEqual(restored.savedRequest(submission.id)?.outcome, outcome)
        let recovered = try await client.createTask(submission)
        XCTAssertEqual(recovered, outcome)
        let opened = await restored.openTask(savedRequestID: submission.id)
        let task = try XCTUnwrap(opened, restored.error ?? "Task not available")
        XCTAssertEqual(try store.load().text, "A separate idea that must not be submitted")
        XCTAssertTrue(try store.load().savedRequests.isEmpty)
        XCTAssertEqual(task.primarySession, outcome.receipt?.sessionId)
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
        await workspace.open(task)
        try await wait("Created Session did not become viewable") { workspace.state == .viewing }
        workspace.requestControl()
        try await wait("Created Session did not accept keyboard control") { workspace.state == .controlling }
        controller.update(theme: .light)
        controller.terminal.insertText("created from phone\n")
        try await wait("Created provider did not receive native input") { workspace.state == .ended }
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

        await workspace.restoreForegroundSession()
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

    private func wait(_ reason: String, seconds: TimeInterval = 10, until predicate: () -> Bool) async throws {
        let deadline = Date().addingTimeInterval(seconds)
        while !predicate() {
            guard Date() < deadline else { throw RegistryQueryError(reason) }
            try await Task.sleep(for: .milliseconds(20))
        }
    }
}
