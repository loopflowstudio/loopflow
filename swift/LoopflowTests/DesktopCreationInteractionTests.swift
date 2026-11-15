#if os(macOS)
import Foundation
import ViewInspector
import XCTest
@testable import Loopflow
@testable import LoopflowMac

extension ClientWorkQueryTests {
    func testDesktopStartSelectsOperationBeforeHeldSaveAndLateFilingLeavesBSelected() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let hold = HeldIntentDisk()
        let server = CreationMachine()
        let disk = ClientWorkStore(directory: root, prepareDirectory: { _ in hold.enter() }, machine: { "machine" },
            read: { _ in "null" }, observe: { server.observe($0) }, send: { server.send($0) })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { hold.release(); owner.shutdown(); server.finish() }
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query, repoPath: "/src/loopflow")
        model.useInteractionForTesting(owner)
        model.applyFixture(roadmap: .available(fixture.roadmap), waves: .available(fixture.waves),
            workActivity: .loading, repos: [])
        model.select(.task(id: "issue-now"))
        model.navigation.palette = .newTask
        let operation = try XCTUnwrap(model.beginTask(request: "Submitted words", wave: nil, graph: "code"))
        XCTAssertNil(model.navigation.palette)
        XCTAssertTrue(model.navigation.startingTask === operation)
        XCTAssertNil(model.selection, "Start immediately leaves the previous Task highlight")
        XCTAssertNil(operation.receipt)
        XCTAssertEqual(operation.directory, "/src/loopflow")
        let view = StartingTaskView(model: model, launch: operation)
        _ = try view.inspect().find(text: "Submitted words")
        XCTAssertThrowsError(try view.inspect().find(button: "Start"))
        XCTAssertTrue(server.submissions.isEmpty)
        var entered = hold.entered.makeAsyncIterator()
        _ = await entered.next()
        model.select(.task(id: "issue-review"))
        XCTAssertEqual(model.selection, .task(id: "issue-review"))
        hold.release()
        await owner.persistPending()
        var observations = server.observations.makeAsyncIterator()
        _ = await observations.next()
        server.publish(try creationFrame(id: operation.id))
        await waitForCreation(operation) { operation.receipt != nil }
        XCTAssertEqual(model.selection, .task(id: "issue-review"))
        XCTAssertNil(model.navigation.startingTask)
        XCTAssertEqual(owner.creations.first?.receipt?.task.id, "created")
        XCTAssertNil(owner.creations.first?.receipt?.sessionId)
        server.publish(try creationFrame(id: operation.id, delivery: "finished", revision: 2, session: "exact-session"))
        await waitForCreation(operation) { operation.receipt?.sessionId != nil }
        XCTAssertEqual(model.selection, .task(id: "issue-review"))
        XCTAssertTrue(server.submissions.isEmpty)
    }
}
#endif
