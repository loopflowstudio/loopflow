import Foundation
import Testing
@testable import Loopflow

func operationTestFrame(id: String = "same", delivery: String? = "finished", epoch: String = "epoch-a",
                        revision: Int = 1, error: String? = nil, result: JSONValue? = .object(["id": .string("original")])) throws -> SubscriptionReading<OperationReceipt> {
    let body: [String: Any]? = delivery.map { delivery in
        ["request_id": id, "kind": "task.create", "fingerprint": NSNull(),
         "submitted_fingerprint": "content", "validation": "bound", "delivery": delivery,
         "revision": revision, "result": result.map { try! JSONSerialization.jsonObject(with: JSONEncoder().encode($0)) } ?? NSNull(),
         "error": error as Any? ?? NSNull(), "commits": [], "planning": NSNull(), "created_at": 1, "updated_at": 2]
    }
    let frame: [String: Any] = ["answers": NSNull(), "unavailable": NSNull(), "body": body as Any? ?? NSNull(),
        "stamp": ["machine": "machine", "epoch": epoch, "schema": ServerProtocol.revision, "query": "operation-query",
                  "complete": true, "observations": NSNull(), "revisions": ["planning": 1, "sessions": 1, "graphs": 1, "processes": 1, "usage": 1]]]
    return try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self, from: JSONSerialization.data(withJSONObject: frame))
}

/// Transport fixture whose admission can be held independently of its receipt.
final class OperationServer: @unchecked Sendable {
    private let lock = NSLock()
    private var current: SubscriptionReading<OperationReceipt>
    private var watches: [UUID: AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error>.Continuation] = [:]
    private var sent: [Data] = []
    var submissions: [Data] { lock.withLock { sent } }
    var watchCount: Int { lock.withLock { watches.count } }
    let entered: AsyncStream<Void>
    private let began: AsyncStream<Void>.Continuation
    let observations: AsyncStream<Void>
    private let observing: AsyncStream<Void>.Continuation
    private let finishOnSend: Bool

    init(frame: SubscriptionReading<OperationReceipt>? = nil, finishOnSend: Bool = true) throws {
        current = try frame ?? operationTestFrame(delivery: nil)
        self.finishOnSend = finishOnSend
        (entered, began) = AsyncStream.makeStream()
        (observations, observing) = AsyncStream.makeStream()
    }

    func observe(_ id: String) -> ClientWorkStore.ReceiptWatch {
        let key = UUID()
        let (stream, continuation) = AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error>.makeStream()
        lock.withLock { watches[key] = continuation; continuation.yield(current) }
        observing.yield(())
        return ServerWatch(updates: stream, checkpoint: { _ in }, cancel: { [self] in
            lock.withLock { watches.removeValue(forKey: key)?.finish() }
        })
    }

    func send(_ envelope: Data) throws {
        let value = try #require(JSONSerialization.jsonObject(with: envelope) as? [String: Any])
        #expect(value["kind"] as? String == "operation.submit")
        #expect(value["epoch"] as? String != nil)
        let command = try #require(value["command"] as? [String: Any])
        #expect(command["request_id"] as? String == value["request_id"] as? String)
        lock.withLock { sent.append(envelope) }
        began.yield(())
        if finishOnSend {
            let (epoch, revision) = lock.withLock { (current.stamp!.epoch, (current.body?.revision ?? 0) + 1) }
            publish(try operationTestFrame(epoch: epoch, revision: Int(revision)))
        }
    }

    func publish(_ frame: SubscriptionReading<OperationReceipt>) {
        lock.withLock {
            current = frame
            for continuation in watches.values { continuation.yield(frame) }
        }
    }

    func finish() { lock.withLock { for continuation in watches.values { continuation.finish() } } }
}

func operationPlanningTestFrame(delivered: Bool, diagnostic: String? = "planning response lost") throws -> SubscriptionReading<OperationReceipt> {
    let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("rust/loopflow/tests/fixtures/dto/server/operation_planning.json")
    let result = try #require(try RawJSON(Data(contentsOf: fixture)).fields()["result"])
    var receipt = try #require(JSONSerialization.jsonObject(with: result.bytes) as? [String: Any])
    receipt["revision"] = delivered ? 15 : 14
    receipt["error"] = diagnostic as Any? ?? NSNull()
    var commits = try #require(receipt["commits"] as? [[String: Any]])
    let references = try #require(commits[0]["planning_changes"] as? [[String: Any]])
    commits[0]["planning_changes"] = [references[0]]
    receipt["commits"] = commits
    var planning = try #require(receipt["planning"] as? [String: Any])
    var fence = try #require(planning["fence"] as? [String: Any])
    fence["revisions"] = ["planning": delivered ? 13 : 12]
    planning["fence"] = fence
    var changes = try #require(planning["changes"] as? [[String: Any]])
    changes = [changes[0]]
    changes[0]["state"] = delivered ? "delivered" : "uncertain"
    changes[0]["error"] = delivered ? NSNull() : "lost delivery acknowledgment" as Any
    planning["changes"] = changes
    receipt["planning"] = planning
    let frame: [String: Any] = ["answers": NSNull(), "unavailable": NSNull(), "body": receipt,
        "stamp": ["machine": "machine_fixture", "epoch": "ab0123456789ab0123456789ab01234567",
                  "schema": ServerProtocol.revision, "query": "operation-query", "complete": true, "observations": NSNull(),
                  "revisions": ["planning": delivered ? 13 : 12, "sessions": 1, "graphs": 1, "processes": 1, "usage": 1]]]
    return try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self,
        from: JSONSerialization.data(withJSONObject: frame))
}


struct ClientWorkStoreTests {
    private func directory() -> URL { FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString) }
    private func client(_ root: URL, _ server: OperationServer) -> ClientWorkStore {
        ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" }, observe: { server.observe($0) }, send: { try server.send($0) })
    }
    private var command: ServedWrite {
        .taskCreate(directory: "/original-repo", requestID: "same", title: nil, request: "Exact submitted words",
                    wave: nil, graph: nil, session: nil, run: true)
    }

    @Test(arguments: [false, true])
    func livePlanningObservationSurvivesSavedReplyReorderingAndRestart(newerFirst: Bool) async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let older = try operationPlanningTestFrame(delivered: false)
        let newer = try operationPlanningTestFrame(delivered: true)
        let server = try OperationServer(frame: newerFirst ? newer : older)
        func client() -> ClientWorkStore {
            ClientWorkStore(directory: root, machine: { "machine_fixture" }, read: { _ in "null" },
                observe: { server.observe($0) }, send: { try server.send($0) })
        }
        let store = client()
        let record = try await store.record(.taskUpdate(directory: "/repo", issue: "LOO-1", notes: "Original edit"),
            requestID: "operation_fixture")
        var watches = server.observations.makeAsyncIterator()
        #expect(try await store.reconcile(record)?.result == older.body?.result)
        _ = await watches.next()
        let following = Task { try await store.observeSaved(record) }
        _ = await watches.next()
        // The saved result and current planning evidence travel independently
        // of Work. A lagging query cannot certify this local edit's stage.
        let stage = try #require(newer.body?.commits.first)
        let stamp = try #require(newer.stamp)
        var laggingJSON = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(stamp)) as? [String: Any])
        laggingJSON["query"] = "task-list"
        laggingJSON["revisions"] = ["planning": 6, "sessions": 1, "graphs": 1, "processes": 1, "usage": 1]
        let lagging = try JSONDecoder().decode(ReadingStamp.self, from: JSONSerialization.data(withJSONObject: laggingJSON))
        #expect(!stage.fence.isCovered(by: lagging, query: "task-list"))
        server.publish(newer)
        server.publish(older)
        server.finish()
        await #expect(throws: (any Error).self) { try await following.value }
        #expect(try await store.receipt(record.id)?.result == older.body?.result)
        #expect(try await store.receipt(record.id)?.error == "planning response lost")
        #expect(try await store.planning(record.id)?.unresolved.isEmpty == true)
        #expect(try await store.pending().first?.payload == record.payload)
        #expect(server.submissions.isEmpty)
        #expect(server.watchCount == 0)

        // Restart with an older first reply: return the saved current evidence,
        // rather than blocking forever or reviving its historical delivery error.
        server.publish(older)
        let restarted = client()
        let recovered = try await restarted.reconcile(record)
        #expect(recovered?.planning == newer.body?.planning)
        #expect(try await restarted.planning(record.id)?.unresolved.isEmpty == true)
        #expect(recovered?.result == older.body?.result)
        #expect(server.submissions.isEmpty)
    }

    @Test func readOnlyObservationNeverReplaysMissingOrResetReceipts() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let known = try operationPlanningTestFrame(delivered: true)
        let server = try OperationServer(frame: known)
        let store = ClientWorkStore(directory: root, machine: { "machine_fixture" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { try server.send($0) })
        let record = try await store.record(.taskUpdate(directory: "/repo", issue: "LOO-1", notes: "Original edit"),
            requestID: "operation_fixture")
        var watches = server.observations.makeAsyncIterator()
        _ = try await store.reconcile(record)
        #expect(try await store.planning(record.id)?.unresolved.isEmpty == true)
        _ = await watches.next()
        let following = Task { try await store.observeSaved(record) }
        _ = await watches.next()
        let knownStamp = try #require(known.stamp)
        var replacementStamp = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(knownStamp)) as? [String: Any])
        replacementStamp["epoch"] = "replacement"
        let replacement: [String: Any] = ["answers": NSNull(), "unavailable": NSNull(), "body": NSNull(), "stamp": replacementStamp]
        server.publish(try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self,
            from: JSONSerialization.data(withJSONObject: replacement)))
        server.publish(known) // departed epoch
        server.finish()
        await #expect(throws: (any Error).self) { try await following.value }
        #expect(try await store.pending().first?.payload == record.payload)
        #expect(try await store.planning(record.id) == nil)
        try await store.presented(record.id)
        #expect(try await store.recovery(record.id)?.resultPresented == true)
        #expect(try await store.recovery(record.id)?.needsInspection == true)
        #expect(server.submissions.isEmpty)
        #expect(server.watchCount == 0)
    }

    @Test func cancellingReadOnlyObservationKeepsTheUnsentRequestRecoverable() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer()
        let store = client(root, server)
        let record = try await store.record(command, requestID: "same")
        var watches = server.observations.makeAsyncIterator()
        let following = Task { try await store.observeSaved(record) }
        _ = await watches.next()
        following.cancel()
        await #expect(throws: CancellationError.self) { try await following.value }
        #expect(server.submissions.isEmpty)
        #expect(server.watchCount == 0)
        #expect(await !store.isRecovering(record.id))
        #expect(try await store.pending().first?.payload == record.payload)
        // Listening did not mark the request sent or acquire a delivery result.
        #expect(try await store.reconcile(record)?.delivery == .finished)
        #expect(server.submissions.count == 1)
    }

    @Test func saveAdmissionReceiptAndRestartRetainOneOriginalCommand() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer()
        let first = client(root, server)
        let record = try await first.record(command, requestID: "same")
        let restarted = client(root, server)
        #expect(try await restarted.pending().first?.payload == record.payload)
        #expect(try await restarted.reconcile(record)?.delivery == .finished)
        #expect(try await client(root, server).reconcile(record)?.result == .object(["id": .string("original")]))
        #expect(server.submissions.count == 1)
        #expect(server.watchCount == 0)
        try await restarted.presented(record.id)
        #expect(try await restarted.pending().first?.payload == record.payload)
        #expect(try await client(root, server).recovery(record.id)?.resultPresented == true)
    }

    @Test func restartRetainsSupersessionAfterAnOlderReceiptArrives() throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("rust/loopflow/tests/fixtures/dto/server/operation_receipt.json")
        let result = try #require(try RawJSON(Data(contentsOf: fixture)).fields()["result"])
        let resolved = try result.decode(OperationReceipt.self)
        var prior = try #require(JSONSerialization.jsonObject(with: result.bytes) as? [String: Any])
        prior["revision"] = resolved.revision - 1
        prior["commits"] = Array((try #require(prior["commits"] as? [Any])).prefix(1))
        let older = try JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: prior))
        let record = SubmittedOperation(requestID: resolved.requestID, destination: "machine_fixture",
            payload: try JSONEncoder().encode(ServedWrite.taskComplete(directory: "/repo", issue: "LOO-1")),
            version: ServerProtocol.revision, submittedAt: Date())
        do {
            let database = try ClientWorkDatabase(directory: root)
            try database.save(record)
            try database.receive(resolved, epoch: "epoch", id: record.id)
            try database.receive(older, epoch: "epoch", id: record.id)
        }
        let restarted = try ClientWorkDatabase(directory: root)
        let saved = try #require(try restarted.state(record.id))
        #expect(saved.record.payload == record.payload)
        #expect(saved.receipt == resolved)
        #expect(saved.receipt?.commits.last?.resolution == .superseded)
        #expect(saved.receipt?.delivery == .uncertain)
    }

    @Test(arguments: [("operation_planning", 2), ("operation_state_planning", 0)])
    func restartRetainsCurrentDeliveryEvidenceAfterAnOlderSavedReply(fixtureName: String, deliveredIndex: Int) throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("rust/loopflow/tests/fixtures/dto/server/\(fixtureName).json")
        let result = try #require(try RawJSON(Data(contentsOf: fixture)).fields()["result"])
        var prior = try #require(JSONSerialization.jsonObject(with: result.bytes) as? [String: Any])
        let older = try JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: prior))
        prior["planning"] = NSNull()
        let withoutStatus = try JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: prior))
        prior = try #require(JSONSerialization.jsonObject(with: result.bytes) as? [String: Any])
        prior["revision"] = older.revision + 1
        var planning = try #require(prior["planning"] as? [String: Any])
        var changes = try #require(planning["changes"] as? [[String: Any]])
        changes[deliveredIndex]["state"] = "delivered"
        changes[deliveredIndex]["error"] = NSNull()
        planning["changes"] = changes
        prior["planning"] = planning
        let current = try JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: prior))
        let command: ServedWrite = fixtureName == "operation_state_planning"
            ? .taskComplete(directory: "/repo", issue: "LOO-1")
            : .taskUpdate(directory: "/repo", issue: "LOO-1", notes: "Original edit")
        let record = SubmittedOperation(requestID: current.requestID, destination: "machine_fixture",
            payload: try JSONEncoder().encode(command),
            version: ServerProtocol.revision, submittedAt: Date())
        do {
            let database = try ClientWorkDatabase(directory: root)
            try database.save(record)
            try database.receive(current, epoch: "epoch", id: record.id)
            try database.receive(older, epoch: "epoch", id: record.id)
            try database.receive(withoutStatus, epoch: "epoch", id: record.id)
        }
        let restarted = try ClientWorkDatabase(directory: root)
        let saved = try #require(try restarted.state(record.id))
        #expect(saved.record.payload == record.payload)
        #expect(saved.receipt == current)
        #expect(saved.receipt?.planning?.changes[deliveredIndex].state == .delivered)
        #expect(saved.receipt?.planning?.changes[deliveredIndex].error == nil)
        #expect(saved.receipt?.result == older.result)
    }

    @Test func heldDomainWorkHasDurableAdmissionAndNoSecondSend() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer(finishOnSend: false)
        let store = client(root, server)
        let submitting = Task { try await store.submit(command, requestID: "same") }
        for await _ in server.entered { break }
        let saved = try #require(await store.pending().first)
        #expect(try JSONDecoder().decode(JSONValue.self, from: saved.payload) == JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(command)))
        #expect(try await store.reconcile(saved) == nil)
        #expect(try await store.receipt("same") == nil) // admission alone enables nothing
        server.publish(try operationTestFrame())
        #expect(try await submitting.value == #"{"id":"original"}"#)
        #expect(server.submissions.count == 1)
    }

    @Test func firstSubmissionOwnsRecoveryBeforeItsFirstAwait() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer()
        let (entered, began) = AsyncStream<Void>.makeStream()
        let (held, release) = AsyncStream<Void>.makeStream()
        let store = ClientWorkStore(directory: root, machine: {
            began.yield(())
            for await _ in held { break }
            return "machine"
        }, read: { _ in "null" }, observe: { server.observe($0) }, send: { try server.send($0) })
        let submitting = Task { try await store.submit(command, requestID: "same") }
        for await _ in entered { break }
        #expect(await store.isRecovering("same"))
        let original = SubmittedOperation(requestID: "same", destination: "machine", payload: try JSONEncoder().encode(command),
            version: ServerProtocol.revision, submittedAt: Date())
        #expect(try await store.reconcile(original) == nil)
        release.finish()
        #expect(try await submitting.value == #"{"id":"original"}"#)
        #expect(server.submissions.count == 1)
    }

    @Test func saveFailureNeverReachesTheMachine() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        try Data().write(to: root)
        let server = try OperationServer()
        await #expect(throws: (any Error).self) { try await client(root, server).submit(command, requestID: "same") }
        #expect(server.submissions.isEmpty)
    }

    @Test func failedImportRetainsItsSourceAndTheStoreCanOpenAgain() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let source = root.appendingPathComponent("released.json")
        try Data("incomplete journal".utf8).write(to: source)
        let server = try OperationServer()
        let store = client(root, server)
        await #expect(throws: (any Error).self) { try await store.pending() }
        #expect(FileManager.default.fileExists(atPath: source.path))
        #expect(server.submissions.isEmpty)
        try FileManager.default.removeItem(at: source)
        let saved = try await store.record(command, requestID: "same")
        #expect(try await store.pending().first?.payload == saved.payload)
    }

    @Test func importedSentJournalAndChangedEpochNeverAuthorizeMissingReceiptReplay() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let record = SubmittedOperation(requestID: "same", destination: "machine", payload: try JSONEncoder().encode(command), version: 5, submittedAt: Date())
        try JSONEncoder().encode(record).write(to: root.appendingPathComponent("released.json"))
        let server = try OperationServer()
        let store = client(root, server)
        #expect(try await store.pending().first?.payload == record.payload)
        #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent("released.json").path))
        await #expect(throws: (any Error).self) { try await store.reconcile(record) }
        #expect(server.submissions.isEmpty)
        #expect(try await store.pending().count == 1)
    }

    @Test func lostAdmissionRecoversByLookupAndEpochResetCannotResendIt() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer()
        let failing = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" }, observe: { server.observe($0) }, send: { _ in
            throw RegistryQueryError("lost admission answer")
        })
        await #expect(throws: OperationSubmissionError.self) { try await failing.submit(command, requestID: "same") }
        let record = try #require(await failing.pending().first)
        // Same lifetime: retrying admission uses exactly the saved content.
        #expect(try await client(root, server).reconcile(record)?.delivery == .finished)
        #expect(server.submissions.count == 1)
        let reset = try OperationServer(frame: operationTestFrame(delivery: nil, epoch: "replacement"))
        await #expect(throws: (any Error).self) { try await client(root, reset).reconcile(record) }
        #expect(reset.submissions.isEmpty)
        // Once replacement was observed, returning to the departed lifetime is
        // not a same-lifetime admission retry, including after another restart.
        await #expect(throws: (any Error).self) { try await client(root, server).reconcile(record) }
        #expect(server.submissions.count == 1)
    }

    @Test func uncertainOutcomeSurvivesPresentationAndRestart() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer(frame: operationTestFrame(delivery: "uncertain", error: "Graph admission interrupted"))
        let store = client(root, server)
        let record = try await store.record(command, requestID: "same")
        #expect(try await store.reconcile(record)?.delivery == .uncertain)
        try await store.presented(record.id)
        #expect(try await store.recovery(record.id)?.resultPresented == true)
        #expect(try await store.recovery(record.id)?.needsInspection == true)
        #expect(try await store.pending().count == 1)
        #expect(server.submissions.isEmpty)
    }

    @Test func otherMachineAndUnavailableReceiptNeverSend() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer()
        let store = client(root, server)
        let record = try await store.record(command, requestID: "same")
        let originalStamp = try #require(operationTestFrame(delivery: nil).stamp)
        var stamp = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(originalStamp)) as? [String: Any])
        var wire: [String: Any] = ["body": NSNull(), "answers": NSNull(), "unavailable": NSNull()]
        stamp["machine"] = "other"
        wire["stamp"] = stamp
        server.publish(try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self, from: JSONSerialization.data(withJSONObject: wire)))
        let other = ClientWorkStore(directory: root, machine: { "other" }, read: { _ in "null" }, observe: { server.observe($0) }, send: { try server.send($0) })
        await #expect(throws: (any Error).self) { try await other.reconcile(record) }
        #expect(server.submissions.isEmpty)
    }

    @Test func changedCommandCannotReplaceASavedSubmission() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer()
        let store = client(root, server)
        let original = try await store.record(command, requestID: "same")
        await #expect(throws: (any Error).self) {
            try await store.record(.taskComplete(directory: "/repo", issue: "different"), requestID: "same")
        }
        #expect(try await store.pending().first?.payload == original.payload)
    }

    @Test func presentationSurvivesEpochReplacementAndRestartWithoutCertifyingOldEffects() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer(frame: operationTestFrame())
        let store = client(root, server)
        let original = try await store.record(command, requestID: "same")
        _ = try await store.reconcile(original)
        try await store.presented(original.id)
        server.publish(try operationTestFrame(delivery: nil, epoch: "replacement"))
        await #expect(throws: (any Error).self) { try await store.reconcile(original) }
        let restarted = client(root, server)
        server.publish(try operationTestFrame()) // departed epoch answers first
        await #expect(throws: (any Error).self) { try await restarted.reconcile(original) }
        let recovery = try #require(await restarted.recovery(original.id))
        #expect(recovery.resultPresented)
        #expect(recovery.needsInspection)
        #expect(!recovery.receiptAvailable)
        #expect(recovery.planning == .unavailable)
        #expect(try await restarted.pending().first?.payload == original.payload)
        #expect(server.submissions.isEmpty)
    }

    @Test func replacementReceiptCannotEraseTheOriginalResultOrDiagnostic() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let original = try operationTestFrame(error: "Attachment interrupted")
        let server = try OperationServer(frame: original)
        let store = client(root, server)
        let record = try await store.record(command, requestID: "same")
        _ = try await store.reconcile(record)
        try await store.presented(record.id)
        server.publish(try operationTestFrame(delivery: "queued", epoch: "replacement", result: nil))
        await #expect(throws: (any Error).self) { try await store.reconcile(record) }
        let restarted = client(root, server)
        #expect(try await restarted.receipt(record.id)?.result == original.body?.result)
        #expect(try await restarted.recovery(record.id)?.diagnostic == "Attachment interrupted")
        #expect(try await restarted.recovery(record.id)?.resultPresented == true)
        #expect(try await restarted.recovery(record.id)?.receiptAvailable == false)
        #expect(server.submissions.isEmpty)
    }

    @Test func domainProofControlsCreationContinuation() async throws {
        for disposition in ["continue", "inspect"] {
            let root = directory()
            defer { try? FileManager.default.removeItem(at: root) }
            let server = try OperationServer(frame: operationTestFrame(delivery: "uncertain"))
            let domain = #"{"request_id":"same","kind":"task.create","milestone":"filed","task_id":"original","session_id":null,"result":null,"disposition":"\#(disposition)","explanation":"Domain recovery","run":true,"artifact_key":null,"launch":null,"created_at":1,"updated_at":2}"#
            let store = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in domain },
                observe: { server.observe($0) }, send: { try server.send($0) })
            let record = try await store.record(command, requestID: "same")
            let receipt = try await store.reconcile(record)
            #expect(receipt?.delivery == (disposition == "continue" ? .finished : .uncertain))
            #expect(server.submissions.count == (disposition == "continue" ? 1 : 0))
        }
    }

    @Test func wrongReceiptAndUnavailableLookupNeverSubmit() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let wrong = try OperationServer(frame: operationTestFrame(id: "another"))
        let store = client(root, wrong)
        let record = try await store.record(command, requestID: "same")
        await #expect(throws: (any Error).self) { try await store.reconcile(record) }
        #expect(wrong.submissions.isEmpty)
        let unavailable = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" }, observe: { _ in
            let stream = AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error> {
                $0.finish(throwing: RegistryQueryError("Machine unavailable"))
            }
            return ServerWatch(updates: stream, checkpoint: { _ in }, cancel: {})
        }, send: { try wrong.send($0) })
        await #expect(throws: (any Error).self) { try await unavailable.reconcile(record) }
        #expect(wrong.submissions.isEmpty)
        #expect(try await unavailable.pending().count == 1)
    }

    @Test func receiptRevisionAndEpochAreIndependentOfArrivalOrder() throws {
        var cursor = ReceiptCursor(machine: "machine")
        let newest = try operationTestFrame(revision: 9)
        #expect(try cursor.receive(newest.body, stamp: #require(newest.stamp)))
        let older = try operationTestFrame(delivery: "queued", revision: 2)
        #expect(try !cursor.receive(older.body, stamp: #require(older.stamp)))
        let reset = try operationTestFrame(delivery: nil, epoch: "new")
        #expect(try cursor.receive(reset.body, stamp: #require(reset.stamp)))
        #expect(try !cursor.receive(newest.body, stamp: #require(newest.stamp)))
        let recovered = try operationTestFrame(epoch: "new", revision: 1)
        #expect(try cursor.receive(recovered.body, stamp: #require(recovered.stamp)))
    }
}
