import Foundation
import Testing
@testable import Loopflow

func operationTestFrame(id: String = "same", delivery: String? = "finished", epoch: String = "epoch-a",
                        revision: Int = 1, error: String? = nil, result: JSONValue? = .object(["id": .string("original")])) throws -> SubscriptionReading<OperationReceipt> {
    let body: [String: Any]? = delivery.map { delivery in
        ["request_id": id, "kind": "task.create", "fingerprint": NSNull(),
         "submitted_fingerprint": "content", "validation": "bound", "delivery": delivery,
         "revision": revision, "result": result.map { try! JSONSerialization.jsonObject(with: JSONEncoder().encode($0)) } ?? NSNull(),
         "error": error as Any? ?? NSNull(), "commits": [], "created_at": 1, "updated_at": 2]
    }
    let frame: [String: Any] = ["answers": NSNull(), "unavailable": NSNull(), "body": body as Any? ?? NSNull(),
        "stamp": ["machine": "machine", "epoch": epoch, "schema": ServerProtocol.revision, "query": "operation-query",
                  "complete": true, "revisions": ["planning": 1, "sessions": 1, "graphs": 1, "processes": 1, "usage": 1]]]
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
    private let finishOnSend: Bool

    init(frame: SubscriptionReading<OperationReceipt>? = nil, finishOnSend: Bool = true) throws {
        current = try frame ?? operationTestFrame(delivery: nil)
        self.finishOnSend = finishOnSend
        (entered, began) = AsyncStream.makeStream()
    }

    func observe(_ id: String) -> ClientWorkStore.ReceiptWatch {
        let key = UUID()
        let (stream, continuation) = AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error>.makeStream()
        lock.withLock { watches[key] = continuation; continuation.yield(current) }
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
}

struct ClientWorkStoreTests {
    private actor Checks {
        private var count = 0
        func next() -> Int { count += 1; return count }
    }

    private func directory() -> URL { FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString) }
    private func client(_ root: URL, _ server: OperationServer) -> ClientWorkStore {
        ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" }, observe: { server.observe($0) }, send: { try server.send($0) })
    }
    private var command: ServedWrite {
        .taskCreate(directory: "/original-repo", requestID: "same", title: nil, request: "Exact submitted words",
                    wave: nil, graph: nil, session: nil, run: true)
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
        #expect(try await restarted.pending().isEmpty)
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
        let reset = try OperationServer(frame: operationTestFrame(delivery: nil, epoch: "replacement"))
        await #expect(throws: (any Error).self) { try await client(root, reset).reconcile(record) }
        #expect(reset.submissions.isEmpty)
        // Same lifetime: retrying admission uses exactly the saved content.
        #expect(try await client(root, server).reconcile(record)?.delivery == .finished)
        #expect(server.submissions.count == 1)
    }

    @Test func uncertainOutcomeSurvivesPresentationAndRestart() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer(frame: operationTestFrame(delivery: "uncertain", error: "Graph admission interrupted"))
        let store = client(root, server)
        let record = try await store.record(command, requestID: "same")
        #expect(try await store.reconcile(record)?.delivery == .uncertain)
        await #expect(throws: (any Error).self) { try await store.presented(record.id) }
        #expect(try await store.pending().count == 1)
        #expect(server.submissions.isEmpty)
    }

    @Test func otherMachineAndUnavailableReceiptNeverSend() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer()
        let store = client(root, server)
        let record = try await store.record(command, requestID: "same")
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

    @Test func epochChangeDuringPresentationKeepsTheUnresolvedRequest() async throws {
        let root = directory()
        defer { try? FileManager.default.removeItem(at: root) }
        let server = try OperationServer(frame: operationTestFrame())
        let checks = Checks()
        let (entered, began) = AsyncStream<Void>.makeStream()
        let (held, release) = AsyncStream<Void>.makeStream()
        let store = ClientWorkStore(directory: root, machine: {
            if await checks.next() == 3 {
                began.yield(())
                for await _ in held { break }
            }
            return "machine"
        }, read: { _ in "null" }, observe: { server.observe($0) }, send: { try server.send($0) })
        let original = try await store.record(command, requestID: "same")
        _ = try await store.reconcile(original)
        let presentation = Task { try await store.presented("same") }
        for await _ in entered { break }
        server.publish(try operationTestFrame(delivery: nil, epoch: "replacement"))
        await #expect(throws: (any Error).self) { try await store.reconcile(original) }
        release.finish()
        await #expect(throws: (any Error).self) { try await presentation.value }
        #expect(try await store.pending().count == 1)
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
