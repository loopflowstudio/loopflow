#if os(macOS)
import Foundation
import ViewInspector
import XCTest
@testable import Loopflow
@testable import LoopflowMac

extension ClientWorkQueryTests {
    func testCreationSaveFailureOffersOneRetryForTheSameIDAndKeepsWordsEditable() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let fault = IntentDiskFault()
        let disk = ClientWorkStore(directory: root, prepareDirectory: { _ in try fault.check() }, machine: { "machine" },
            read: { _ in "null" }, observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("No admission evidence yet") })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query, repoPath: "/repo")
        model.useInteractionForTesting(owner)
        let operation = try XCTUnwrap(model.beginTask(request: "Keep these words", wave: nil, graph: nil))
        await owner.persistPending()
        let view = StartingTaskView(model: model, launch: operation)
        _ = try view.inspect().find(button: "Edit request")
        XCTAssertEqual(model.pendingOperations.count, 1)
        XCTAssertEqual(model.pendingOperations.map(\.id), [operation.id])
        fault.recover()
        try view.inspect().find(button: "Try again").tap()
        await owner.persistPending()
        let pending = try await disk.pending()
        XCTAssertEqual(pending.map(\.id), [operation.id])
        XCTAssertFalse(model.canEditCreation(operation.id))
        XCTAssertThrowsError(try view.inspect().find(button: "Edit request"))
        XCTAssertThrowsError(try view.inspect().find(button: "Check request"))
    }

    func testRestartPresentsAnUncertainSavedCreationWithoutResubmitting() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationMachine()
        let disk = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { server.send($0) })
        let owner = WorkInteraction(machine: "machine", store: disk)
        let operation = owner.createTask(request: "Saved original words", directory: "/repo") { _ in }
        await owner.persistPending()
        var observations = server.observations.makeAsyncIterator()
        _ = await observations.next()
        server.publish(try creationFrame(id: operation.id, delivery: "uncertain", session: "original-session", diagnostic: "Admission unconfirmed"))
        await waitForCreation(operation) { operation.receipt != nil }
        owner.shutdown()
        let restarted = WorkInteraction(machine: "machine", store: disk)
        defer { restarted.shutdown(); server.finish() }
        restarted.start()
        await restarted.refreshFromStore()
        let restored = try XCTUnwrap(restarted.listedOperations.first)
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query, repoPath: "/repo")
        model.useInteractionForTesting(restarted)
        model.selectCreation(restored)
        let view = StartingTaskView(model: model, launch: restored)
        _ = try view.inspect().find(text: "Saved original words")
        _ = try view.inspect().find(button: "Check request")
        XCTAssertThrowsError(try view.inspect().find(button: "Try again"))
        XCTAssertEqual(restored.id, operation.id)
        XCTAssertEqual(restored.sessionID, "original-session")
        XCTAssertTrue(server.submissions.isEmpty)
    }

    func testStartWithoutRepositoryKeepsEditorAndCreatesNothing() throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query, repoPath: nil)
        model.navigation.newTaskDraft = .init(request: "Keep these words", wave: nil, graph: nil)
        model.navigation.palette = .newTask
        XCTAssertNil(model.beginTask(request: "Keep these words", wave: nil, graph: nil))
        XCTAssertEqual(model.navigation.newTaskDraft?.request, "Keep these words")
        XCTAssertNil(model.navigation.startingTask)
        _ = try NewTaskSheet(model: model).inspect().find(text: "Open a repository to start a Task.")
    }

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
        XCTAssertEqual(model.pendingOperations.count, 1)
        await model.presentedTaskResult("created")
        XCTAssertTrue(model.pendingOperations.isEmpty)
        XCTAssertEqual(model.selection, .task(id: "issue-review"))
    }
}
#endif
