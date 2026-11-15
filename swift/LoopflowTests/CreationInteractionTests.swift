import Foundation
import Observation
import XCTest
@testable import Loopflow

/// These cases run on both hosts through the production intent/outbox path.
func creationFrame(id: String, delivery: String = "dispatched", revision: Int = 1,
                   session: String? = nil, diagnostic: String? = nil, taskID: String = "created", identifier: String = "NEW-1") throws -> SubscriptionReading<OperationReceipt> {
    let result: [String: Any] = ["task": ["id": taskID, "identifier": identifier, "title": "Original work"],
        "graph": "code", "session_id": session as Any? ?? NSNull(), "milestone": session == nil ? "filed" : "admitted"]
    return try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self, from: JSONSerialization.data(withJSONObject: [
        "answers": NSNull(), "unavailable": NSNull(),
        "stamp": ["machine": "machine", "epoch": "epoch", "schema": ServerProtocol.revision, "query": "operation",
            "complete": true, "revisions": ["planning": 7, "sessions": 7, "graphs": 7, "processes": 7, "usage": 7]],
        "body": ["request_id": id, "kind": "task.create", "fingerprint": "bound", "submitted_fingerprint": "content",
            "validation": "bound", "delivery": delivery, "revision": revision,
            "result": session == nil ? NSNull() as Any : result as Any, "error": diagnostic as Any? ?? NSNull(),
            "commits": [["stage": "task.filed", "resolution": "committed", "resources": ["created"],
                "fence": ["machine": "machine", "epoch": "epoch", "schema": ServerProtocol.revision, "revisions": ["planning": 7]],
                "result": result, "planning_effect": "none", "planning_changes": []]],
            "planning": NSNull(), "created_at": 1, "updated_at": 2]
    ]))
}

extension ClientWorkQueryTests {
    func testCreationPublishesBeforeHeldSaveAndSurvivesDepartureAndRestart() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let hold = HeldIntentDisk()
        let server = CreationMachine()
        let disk = ClientWorkStore(directory: directory, prepareDirectory: { _ in hold.enter() },
            machine: { "machine" }, read: { _ in "null" }, observe: { server.observe($0) }, send: { server.send($0) })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { hold.release(); owner.shutdown(); server.finish() }
        var selected = "old"
        let operation = owner.createTask(request: "  Exact words\n", directory: "/repo") { selected = $0.id }
        XCTAssertEqual(selected, operation.id)
        XCTAssertEqual(owner.creations.first?.request, "  Exact words\n")
        XCTAssertNil(operation.receipt)
        XCTAssertTrue(server.submissions.isEmpty)
        var entered = hold.entered.makeAsyncIterator()
        _ = await entered.next()
        selected = "B"
        hold.release()
        await owner.persistPending()
        let saved = try await disk.pending()
        XCTAssertEqual(saved.first?.id, operation.id)
        var observed = server.observations.makeAsyncIterator()
        _ = await observed.next()
        let missing = try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self, from: JSONSerialization.data(withJSONObject: [
            "answers": NSNull(), "unavailable": NSNull(), "body": NSNull(),
            "stamp": JSONSerialization.jsonObject(with: JSONEncoder().encode(stamp("operation", revision: 6)))]))
        var sends = server.sends.makeAsyncIterator()
        server.publish(missing)
        _ = await sends.next()
        XCTAssertEqual(server.submissions.count, 1)
        let envelope = try JSONDecoder().decode(JSONValue.self, from: XCTUnwrap(server.submissions.first))
        XCTAssertEqual(envelope["request_id"], .string(operation.id))
        XCTAssertEqual(envelope["command"]["request"], .string("  Exact words\n"))
        server.publish(try creationFrame(id: operation.id))
        await waitForCreation(operation) { operation.receipt != nil }
        XCTAssertEqual(operation.receipt?.task.id, "created")
        XCTAssertNil(operation.receipt?.sessionId, "Filing cannot invent admission or attachment")
        XCTAssertEqual(selected, "B")
        server.publish(try creationFrame(id: operation.id, delivery: "uncertain", revision: 2,
            session: "exact-session", diagnostic: "Attachment remains unconfirmed"))
        await waitForCreation(operation) { operation.receipt?.sessionId != nil }
        XCTAssertEqual(selected, "B")
        XCTAssertEqual(operation.receipt?.sessionId, "exact-session")
        owner.shutdown()
        let restarted = WorkInteraction(machine: "machine", store: disk)
        defer { restarted.shutdown() }
        restarted.start()
        await restarted.refreshFromStore()
        let restored = try XCTUnwrap(restarted.creations.first)
        XCTAssertEqual(restored.id, operation.id)
        XCTAssertEqual(restored.request, "  Exact words\n")
        XCTAssertEqual(restored.receipt?.sessionId, "exact-session")
        XCTAssertEqual(server.submissions.count, 1, "Known creation results never require another create")
    }

    func testLegacyImportIsAtomicKeepsSendHistoryAndNeverReplaysMissingEvidence() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        func record(_ id: String, text: String = "Original words") throws -> SubmittedOperation {
            SubmittedOperation(requestID: id, destination: "machine", payload: try JSONEncoder().encode(
                ServedWrite.taskCreate(directory: "/repo", requestID: id, title: nil, request: text,
                    wave: nil, graph: nil, session: nil, run: true)), version: ServerProtocol.revision, submittedAt: Date(timeIntervalSince1970: 1))
        }
        let original = try record("saved")
        let database = try ClientWorkDatabase(directory: directory)
        try database.save(original)
        try database.markSent(original.id, epoch: "epoch")
        _ = try database.observeEpoch("epoch", id: original.id)
        let receipt = try XCTUnwrap(creationFrame(id: original.id, delivery: "finished", session: "exact-session").body)
        try database.receive(receipt, epoch: "epoch", id: original.id)
        let outcome: JSONValue = .object(["task_id": .string("created"), "state": .string("started")])
        try database.importLegacy([LegacyOperation(record: original, outcome: outcome)])
        try database.importLegacy([LegacyOperation(record: original, outcome: outcome)])
        XCTAssertEqual(try database.state(original.id)?.receipt, receipt)
        XCTAssertEqual(try database.state(original.id)?.sentEpoch, "epoch")
        XCTAssertEqual(try database.state(original.id)?.legacyOutcome, outcome)
        let another = try record("another")
        XCTAssertThrowsError(try database.importLegacy([LegacyOperation(record: another, outcome: nil),
            LegacyOperation(record: record("saved", text: "Conflicting words"), outcome: nil)]))
        XCTAssertNil(try database.state("another"), "A conflict rolls back the entire import")
        XCTAssertEqual(try database.state(original.id)?.record.payload, original.payload)
        try database.presented(original.id)
        try database.importLegacy([LegacyOperation(record: original, outcome: outcome)])
        XCTAssertTrue(try database.pending().isEmpty, "Repeating import after retirement cannot resurrect a request")
        XCTAssertNil(try database.state(original.id), "A retired identity carries no receipt or authored payload")
        let imported = try record("legacy")
        try database.importLegacy([LegacyOperation(record: imported, outcome: outcome)])
        XCTAssertEqual(try ClientWorkDatabase(directory: directory).state(imported.id)?.sentEpoch, "")
        let server = CreationMachine()
        let disk = ClientWorkStore(directory: directory, machine: { "machine" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { server.send($0) })
        let recovering = Task { try await disk.reconcile(imported) }
        var observations = server.observations.makeAsyncIterator()
        _ = await observations.next()
        let empty = try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self, from: JSONSerialization.data(withJSONObject: [
            "answers": NSNull(), "unavailable": NSNull(), "body": NSNull(),
            "stamp": JSONSerialization.jsonObject(with: JSONEncoder().encode(stamp("operation", revision: 7)))]))
        server.publish(empty)
        do { _ = try await recovering.value; XCTFail("Missing historical evidence requires inspection") }
        catch { XCTAssertTrue(error.localizedDescription.contains("database changed")) }
        XCTAssertTrue(server.submissions.isEmpty)
        server.finish()
    }
    func testRecentQueryReopensFromDiskWhileNoServerAnswerCanArrive() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let disk = store(root)
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let query = ServedRead.viewSelect(expression: "state:working")
        let lease = owner.open(query) { _ in }
        try await owner.receive([workingPlan(["a"])], stamp: stamp("query", revision: 6), for: query)
        owner.close(lease)
        let (answers, publish) = AsyncStream<Int>.makeStream()
        defer { publish.finish() }
        let reopened = owner.open(query) { plans in if let count = plans?.first?.taskCount { publish.yield(count) } }
        defer { owner.close(reopened) }
        var readings = answers.makeAsyncIterator()
        let count = await readings.next()
        XCTAssertEqual(count, 1)
    }

    func testPresentingCreationKeepsAHeldQueryColdAcrossRestart() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationMachine()
        let disk = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { server.send($0) })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown(); server.finish() }
        let list = ServedRead.viewSelect(expression: "state:working")
        let count = ServedRead.desktopViewGet(directory: "/repo", expression: "state:working")
        _ = owner.open(list) { _ in }; _ = owner.open(count) { _ in }
        try await owner.receive([workingPlan(["a"])], stamp: stamp("list", revision: 6), for: list)
        try await owner.receive([workingPlan(["a"])], stamp: stamp("count", revision: 6), for: count)
        let operation = owner.createTask(request: "Original words", directory: "/repo") { _ in }
        await owner.persistPending()
        var observations = server.observations.makeAsyncIterator()
        _ = await observations.next()
        server.publish(try creationFrame(id: operation.id, delivery: "finished", session: "original-session"))
        await waitForCreation(operation) { operation.receipt?.sessionId != nil }
        try await owner.receive([workingPlan(["a", "created"])], stamp: stamp("list", revision: 7), for: list)
        try await owner.presented(operation.id)
        owner.shutdown()
        let restarted = store(root)
        let currentList = try await restarted.cachedPlans(machine: "machine", for: list)
        let heldCount = try await restarted.cachedPlans(machine: "machine", for: count)
        let pending = try await restarted.pending()
        XCTAssertEqual(currentList?.projection.first?.taskCount, 2)
        XCTAssertNil(heldCount, "Showing the conversation cannot acknowledge an older query")
        XCTAssertTrue(pending.isEmpty, "The query's saved floor outlives command retirement")
        XCTAssertTrue(server.submissions.isEmpty)
    }

    func testCreationAndCompleteSaveFailuresHaveOnePresentationShapeAndIndependentRetry() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let fault = IntentDiskFault()
        let disk = ClientWorkStore(directory: root, prepareDirectory: { _ in try fault.check() }, machine: { "machine" },
            read: { _ in "null" }, observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("No receipt permits a send") })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let creation = owner.createTask(request: "Exact words", directory: "/repo") { _ in }
        let work = WorkWorkspaceKey(repository: "repo", kind: .task, id: "a")
        let completion = owner.complete(work, issue: "a", directory: "/repo", selected: nil, visible: [work]) { _ in XCTFail() }
        await owner.persistPending()
        XCTAssertEqual(owner.listedOperations.map(\.id), [creation.id, completion])
        let complete = try XCTUnwrap(owner.operations.first { $0.id == completion })
        XCTAssertEqual(creation.presentation().status, complete.presentation().status)
        XCTAssertEqual(creation.presentation().actions, [.tryAgain, .edit])
        XCTAssertEqual(complete.presentation().actions, [.tryAgain])
        XCTAssertFalse(owner.isCompleting(issue: "a", directory: "/repo"), "An unsaved failure withdraws only its overlay")
        fault.recover()
        owner.retry(completion)
        XCTAssertTrue(owner.isCompleting(issue: "a", directory: "/repo"))
        await owner.persistPending()
        XCTAssertTrue(complete.saved)
        XCTAssertEqual(complete.presentation().actions, [])
        XCTAssertEqual(creation.presentation().actions, [.tryAgain, .edit])
        let saved = try await disk.pending()
        XCTAssertEqual(saved.map(\.id), [completion])
        owner.editCreation(creation.id) { XCTAssertEqual($0.request, "Exact words") }
        XCTAssertTrue(owner.listedOperations.isEmpty)
    }

    func testUnsavedCreationCanReturnItsWordsToTheEditorWithoutSending() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let disk = ClientWorkStore(directory: root, prepareDirectory: { _ in throw RegistryQueryError("Disk unavailable") },
            machine: { "machine" }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { $0.finish() }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("An unsaved creation cannot send") })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let operation = owner.createTask(request: "Exact unsaved words", directory: "/original") { _ in }
        await owner.persistPending()
        XCTAssertFalse(operation.saved)
        XCTAssertEqual(operation.presentation().actions, [.tryAgain, .edit])
        XCTAssertEqual(owner.listedOperations.map(\.id), [operation.id])
        XCTAssertTrue(owner.canEditCreation(operation.id))
        var edited: PendingOperation?
        owner.editCreation(operation.id) { edited = $0 }
        XCTAssertTrue(edited === operation)
        XCTAssertEqual(edited?.request, "Exact unsaved words")
        XCTAssertEqual(edited?.destination, "machine")
        XCTAssertTrue(owner.creations.isEmpty)
        XCTAssertTrue(owner.listedOperations.isEmpty)
    }

    func testShownFiledResultHidesNormalPreparationAndReappearsIfAdmissionFails() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationMachine()
        let disk = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { server.send($0) })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown(); server.finish() }
        let operation = owner.createTask(request: "Original words", directory: "/repo") { _ in }
        await owner.persistPending()
        var observations = server.observations.makeAsyncIterator()
        _ = await observations.next()
        server.publish(try creationFrame(id: operation.id))
        await waitForCreation(operation) { operation.receipt != nil }
        XCTAssertTrue(operation.presentation().actions.isEmpty, "Normal Graph preparation needs no recovery controls")
        try await owner.presented(.task("created"))
        XCTAssertTrue(owner.listedOperations.isEmpty)
        let pending = try await disk.pending()
        XCTAssertFalse(pending.isEmpty, "The domain observer outlives its redundant prompt")
        server.publish(try creationFrame(id: operation.id, delivery: "uncertain", revision: 2, diagnostic: "Admission failed"))
        await waitForCreation(operation) { operation.error != nil }
        XCTAssertEqual(owner.listedOperations.map(\.id), [operation.id])
        XCTAssertEqual(operation.presentation().actions, [.check, .inspect])
        XCTAssertTrue(server.submissions.isEmpty)
    }

    func testOrdinaryTaskPresentationHidesRequestAcrossRestartWithoutResolvingPreparation() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationMachine()
        let disk = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { server.send($0) })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown(); server.finish() }
        let operation = owner.createTask(request: "Original words", directory: "/repo") { _ in }
        await owner.persistPending()
        var observations = server.observations.makeAsyncIterator()
        _ = await observations.next()
        server.publish(try creationFrame(id: operation.id, delivery: "finished", session: "original-session"))
        await waitForCreation(operation) { operation.receipt != nil }
        XCTAssertEqual(owner.listedOperations.count, 1)
        try await owner.presented(.task("created"))
        XCTAssertTrue(owner.listedOperations.isEmpty)
        let restarted = WorkInteraction(machine: "machine", store: disk)
        defer { restarted.shutdown() }
        restarted.start()
        await restarted.refreshFromStore()
        XCTAssertTrue(restarted.listedOperations.isEmpty)
        XCTAssertTrue(server.submissions.isEmpty)
    }

    func testSubmittedDraftIdentitySurvivesRetirementAndRejectsASecondCommand() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let database = try ClientWorkDatabase(directory: root)
        func record(_ id: String) throws -> SubmittedOperation {
            SubmittedOperation(requestID: id, destination: "machine", payload: try JSONEncoder().encode(
                ServedWrite.taskCreate(directory: "/repo", requestID: id, title: nil, request: "Original words",
                    wave: nil, graph: nil, session: nil, run: true)), version: ServerProtocol.revision,
                submittedAt: Date(), draftID: "original-editor-version")
        }
        try database.save(record("original"))
        _ = try database.observeEpoch("epoch", id: "original")
        try database.receive(XCTUnwrap(creationFrame(id: "original", delivery: "finished", session: "session").body), epoch: "epoch", id: "original")
        try database.presented("original")
        XCTAssertTrue(try database.pending().isEmpty)
        let restarted = try ClientWorkDatabase(directory: root)
        XCTAssertTrue(try restarted.draftSubmitted("original-editor-version"))
        XCTAssertThrowsError(try restarted.save(record("duplicate")))
        XCTAssertTrue(try restarted.pending().isEmpty)
        XCTAssertNil(try restarted.state("original"), "Retirement removes the authored payload")
        try restarted.save(record("original"))
        XCTAssertTrue(try restarted.pending().isEmpty, "A surviving source cannot replay the retired identity")

    }

}

@MainActor
func waitForCreation(_ operation: PendingOperation, until ready: @MainActor () -> Bool) async {
    while !ready() {
        await withCheckedContinuation { continuation in
            withObservationTracking {
                _ = operation.receipt
                _ = operation.error
                _ = operation.recovery
            } onChange: { continuation.resume() }
        }
    }
}

final class CreationMachine: @unchecked Sendable {
    private let lock = NSLock()
    private var watches: [UUID: AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error>.Continuation] = [:]
    private var current: SubscriptionReading<OperationReceipt>?
    private var sent: [Data] = []
    var submissions: [Data] { lock.withLock { sent } }
    let observations: AsyncStream<Void>
    private let observing: AsyncStream<Void>.Continuation
    let sends: AsyncStream<Void>
    private let sending: AsyncStream<Void>.Continuation
    init() {
        (observations, observing) = AsyncStream.makeStream()
        (sends, sending) = AsyncStream.makeStream()
    }
    func observe(_ id: String) -> ClientWorkStore.ReceiptWatch {
        let key = UUID()
        let (stream, continuation) = AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error>.makeStream()
        lock.withLock { watches[key] = continuation; if let current { continuation.yield(current) } }
        observing.yield(())
        return ServerWatch(updates: stream, checkpoint: { _ in }, cancel: { [self] in
            lock.withLock { watches.removeValue(forKey: key)?.finish() }
        })
    }
    func send(_ data: Data) { lock.withLock { sent.append(data) }; sending.yield(()) }
    func publish(_ frame: SubscriptionReading<OperationReceipt>) {
        lock.withLock { current = frame; for watch in watches.values { watch.yield(frame) } }
    }
    func finish() { lock.withLock { for watch in watches.values { watch.finish() } } }
}
