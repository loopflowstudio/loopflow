#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
struct OperationRecoveryPresentationTests {
    @Test func aConversationShowsTheRequestOnceAndProgressBelongsToItsKind() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { _ in fatalError("No delivery in a view inspection") }, send: { _ in })
        let owner = WorkInteraction(machine: "machine", store: store)
        defer { owner.shutdown() }
        let command = ServedWrite.taskCreate(directory: "/repo", requestID: "request", title: nil,
            request: "Keep these words", wave: nil, graph: nil, session: nil, run: true)
        let operation = PendingOperation(id: "request", payload: try JSONEncoder().encode(command), kind: "task.create",
            request: "Keep these words", directory: "/repo", wave: nil, graph: nil, submittedAt: Date(), destination: "machine")
        let conversation = try PendingOperationRow(operation: operation, interaction: owner, edit: { _ in }, context: .conversation).inspect()
        #expect(throws: (any Error).self) { try conversation.find(text: "Keep these words") }
        #expect(try conversation.find(text: "Starting conversation…").string() == "Starting conversation…")
        let list = try PendingOperationRow(operation: operation, interaction: owner, edit: { _ in }).inspect()
        #expect(try list.find(text: "Keep these words").string() == "Keep these words")
        #expect(throws: (any Error).self) { try list.find(text: "Starting conversation…") }
        #expect(OperationDeclaration(kind: "session.new")?.progress == "Starting conversation…")
        #expect(OperationDeclaration(kind: "task.complete")?.progress == "Completing…")
    }

    @Test func waitingListsKeepTitlesAndAttentionKeepsItsActions() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { _ in fatalError("No delivery in a view inspection") }, send: { _ in })
        let owner = WorkInteraction(machine: "machine", store: store)
        defer { owner.shutdown() }
        let command = ServedWrite.taskComplete(directory: "/repo", issue: "LOO-1")
        let operation = PendingOperation(id: "request", payload: try JSONEncoder().encode(command), kind: "task.complete",
            request: "", directory: "/repo", wave: nil, graph: nil, submittedAt: Date(), destination: "machine")
        operation.markSaved()
        operation.waitingForConnection = true
        let list = try PendingOperationRow(operation: operation, interaction: owner, edit: { _ in }).inspect()
        #expect(try list.find(text: "Complete Task · LOO-1").string() == "Complete Task · LOO-1")
        #expect(throws: (any Error).self) { try list.find(text: "Waiting for connection") }
        let conversation = try PendingOperationRow(operation: operation, interaction: owner, edit: { _ in }, context: .conversation).inspect()
        #expect(try conversation.find(text: "Waiting for connection").string() == "Waiting for connection")
        operation.fail("Inspect this request")
        #expect(operation.presentation().status == .needsAttention(["Inspect this request"]))
        #expect(operation.presentation().actions.contains(.check))
        let unsaved = PendingOperation(id: "unsaved", payload: try JSONEncoder().encode(command), kind: "task.create",
            request: "Words", directory: "/repo", wave: nil, graph: nil, submittedAt: Date(), destination: "machine")
        unsaved.waitingForConnection = true
        unsaved.fail("Disk unavailable")
        #expect(unsaved.presentation().actions == [.tryAgain, .edit])
    }

    @Test func machineWaitingIncludesUnwatchedChangesAndNeverAnotherDestination() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let connection = MachineConnection(machine: "machine") { throw ServerConnectionUnavailable(RegistryQueryError("Offline")) }
        _ = try? await connection.link()
        let store = ClientWorkStore(directory: root, connection: connection, machine: { "machine" }, read: { _ in "null" },
            observe: { _ in fatalError("Presentation needs no receipt watch") }, send: { _ in })
        for destination in ["machine", "other"] {
            try await store.save(SubmittedOperation(requestID: destination, destination: destination,
                payload: JSONEncoder().encode(ServedWrite.taskComplete(directory: "/repo", issue: "LOO-1")),
                version: ServerProtocol.revision, submittedAt: Date()))
        }
        let local = try await store.presentation(machine: "machine", queries: [:])
        #expect(local.waitingForConnection)
        #expect(local.operations.count == 1)
        #expect(local.operations[0].waitingForConnection)
        let other = try await store.presentation(machine: "other", queries: [:])
        #expect(!other.waitingForConnection)
        #expect(other.operations.count == 1)
        #expect(!other.operations[0].waitingForConnection)
    }

    @Test func aPresentedResultKeepsObservingWhileItsPlanningConfirmationIsMissing() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let pending = try operationPlanningTestFrame(delivered: false, diagnostic: nil)
        let delivered = try operationPlanningTestFrame(delivered: true, diagnostic: nil)
        let server = try OperationServer(frame: pending)
        func client() -> ClientWorkStore {
            ClientWorkStore(directory: root, machine: { "machine_fixture" }, read: { _ in "null" },
                observe: { server.observe($0) }, send: { try server.send($0) })
        }
        let submissions = client()
        let record = try await submissions.record(.taskUpdate(directory: "/repo", issue: "LOO-1", notes: "Original edit"),
            requestID: "operation_fixture")
        _ = try await submissions.reconcile(record)
        let recovery = WorkInteraction(machine: "machine_fixture", store: submissions)
        defer { recovery.shutdown() }
        recovery.start()
        var watches = server.observations.makeAsyncIterator()
        _ = await watches.next()
        await recovery.refreshFromStore()
        _ = await watches.next()
        _ = await watches.next()
        try await recovery.presented(record.id)
        #expect(recovery.listedOperations.first?.presentation().status.text?.contains("lost delivery acknowledgment") == true)
        #expect(server.watchCount == 1)

        let changes = await submissions.changes()
        server.publish(delivered)
        for await _ in changes {
            if try await submissions.pending().isEmpty { break }
        }
        for await _ in await submissions.changes() {
            if await !submissions.isRecovering(record.id) { break }
        }
        await recovery.refreshFromStore()
        #expect(recovery.listedOperations.isEmpty)
        #expect(server.watchCount == 0)
        let restarted = client()
        #expect(try await restarted.recovery(record.id) == nil)
        #expect(try await restarted.pending().isEmpty)
        #expect(server.submissions.isEmpty)
    }

    @Test func successfulCreationHasNoPermanentInspectRowAndStopsItsObserver() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_create_receipt.json")
        let result = try #require(JSONDecoder().decode([JSONValue].self, from: Data(contentsOf: fixture)).first)
        let original = try operationTestFrame(result: result)
        let savedReceipt = try #require(original.body)
        var body = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(savedReceipt)) as? [String: Any])
        body["commits"] = [["resolution": "committed", "stage": "task.filed", "resources": [],
            "fence": ["machine": "machine", "epoch": "epoch-a", "schema": ServerProtocol.revision,
                      "revisions": ["planning": 7, "sessions": 7]],
            "result": try JSONSerialization.jsonObject(with: JSONEncoder().encode(result)),
            "planning_effect": "none", "planning_changes": []]]
        let frame = try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self,
            from: JSONSerialization.data(withJSONObject: ["answers": NSNull(), "unavailable": NSNull(), "body": body,
                "stamp": try JSONSerialization.jsonObject(with: JSONEncoder().encode(try #require(original.stamp)))]))
        let server = try OperationServer(frame: frame)
        let submissions = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { try server.send($0) })
        let record = try await submissions.record(.taskCreate(directory: "/repo", requestID: "same", title: nil,
            request: "Original request", wave: nil, graph: nil, session: nil, run: true), requestID: "same")
        _ = try await submissions.reconcile(record)
        let owner = WorkInteraction(machine: "machine", store: submissions)
        defer { owner.shutdown() }
        owner.start()
        await owner.refreshFromStore()
        #expect(owner.listedOperations.count == 1)
        // Acknowledge by the actual bound resource; fixtures own their identity.
        if let id = owner.creations.first?.taskID { try await owner.presented(.task(id)) }
        for await _ in await submissions.changes() {
            if await !submissions.isRecovering(record.id) { break }
        }
        await owner.refreshFromStore()
        #expect(owner.listedOperations.isEmpty)
        #expect(try await submissions.pending().isEmpty)
        #expect(server.watchCount == 0)
        #expect(server.submissions.isEmpty)
    }

    @Test func aMountedSessionAcknowledgesItsSavedCommandWithoutAnotherPresentationMap() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let original = try operationTestFrame()
        let originalBody = try #require(original.body)
        var body = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(originalBody)) as? [String: Any])
        body["kind"] = "session.new"
        body["revision"] = 2
        body["commits"] = [["resolution": "committed", "stage": "session.input_replaced", "resources": ["original"],
            "fence": ["machine": "machine", "epoch": "epoch-a", "schema": ServerProtocol.revision, "revisions": ["sessions": 1]],
            "result": ["id": "original"], "planning_effect": "none", "planning_changes": []]]
        let frame = try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self, from: JSONSerialization.data(withJSONObject: [
            "answers": NSNull(), "unavailable": NSNull(), "body": body,
            "stamp": JSONSerialization.jsonObject(with: JSONEncoder().encode(try #require(original.stamp)))]))
        var candidate = body
        candidate["validation"] = "candidate"
        candidate["revision"] = 1
        let candidateFrame = try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self, from: JSONSerialization.data(withJSONObject: [
            "answers": NSNull(), "unavailable": NSNull(), "body": candidate,
            "stamp": JSONSerialization.jsonObject(with: JSONEncoder().encode(try #require(original.stamp)))]))
        let server = try OperationServer(frame: candidateFrame)
        let store = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { _ in Issue.record("A known Session must never be recreated") })
        let record = try await store.record(.sessionNew(directory: "/repo", requestID: "same", id: "original", agent: nil), requestID: "same")
        _ = try await store.reconcile(record)
        let owner = WorkInteraction(machine: "machine", store: store)
        defer { owner.shutdown() }
        owner.start()
        await owner.refreshFromStore()
        #expect(owner.listedOperations.first?.sessionID == nil)
        try await owner.presented(.session("original"))
        #expect(try await store.recovery(record.id)?.resultPresented == false, "A candidate cannot bind presentation to a Session")
        server.publish(frame)
        for await _ in await store.changes() {
            if try await store.receipt(record.id)?.validation == .bound { break }
        }
        await owner.refreshFromStore()
        #expect(owner.listedOperations.map(\.sessionID) == ["original"])
        #expect(owner.listedOperations.first?.presentation().status == .delivering)
        try await owner.presented(.session("another"))
        #expect(try await store.pending().count == 1)
        try await owner.presented(.session("original"))
        #expect(try await store.pending().isEmpty)
        #expect(owner.listedOperations.isEmpty)
    }

    @Test func missingPlanningEvidenceCannotDischargeInspection() async throws {
        let frame = try operationPlanningTestFrame(delivered: true, diagnostic: nil)
        let receipt = try #require(frame.body)
        let noEvidence = OperationRecovery(receipt: receipt, planning: nil,
            resultPresented: true, receiptAvailable: true)
        #expect(noEvidence.resultPresented)
        #expect(noEvidence.needsInspection)
        #expect(noEvidence.planning == .unavailable)
        let partial = OperationPlanningReading(fence: try #require(receipt.planning).fence, changes: [])
        let missingChange = OperationRecovery(receipt: receipt, planning: partial,
            resultPresented: false, receiptAvailable: true)
        #expect(missingChange.needsInspection)
        #expect(missingChange.planning == .unavailable)
    }

    @Test func planningDeliveryCannotHideAnUnclassifiedFailureOrConsumeTheResult() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let older = try operationPlanningTestFrame(delivered: false)
        let newer = try operationPlanningTestFrame(delivered: true)
        let server = try OperationServer(frame: older)
        let submissions = ClientWorkStore(directory: root, machine: { "machine_fixture" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { try server.send($0) })
        let record = try await submissions.record(.taskUpdate(directory: "/repo", issue: "LOO-1", notes: "Original edit"),
            requestID: "operation_fixture")
        var watches = server.observations.makeAsyncIterator()
        _ = try await submissions.reconcile(record)
        _ = await watches.next()
        let recovery = WorkInteraction(machine: "machine_fixture", store: submissions)
        defer { recovery.shutdown() }
        recovery.start()
        await recovery.refreshFromStore()
        #expect(recovery.listedOperations.first?.presentation().actions.contains(.inspect) == true)
        #expect(recovery.listedOperations.first?.presentation().status.text?.contains("lost delivery acknowledgment") == true)
        _ = await watches.next() // saved-result lookup
        _ = await watches.next() // continued observation after returning the result

        let changes = await submissions.changes()
        server.publish(newer)
        for await _ in changes {
            if try await submissions.receipt(record.id)?.revision == newer.body?.revision { break }
        }
        await recovery.refreshFromStore()
        #expect(recovery.listedOperations.first?.presentation().actions.contains(.inspect) == true)
        #expect(recovery.listedOperations.first?.presentation().status.text?.contains("planning response lost") == true)
        #expect(recovery.listedOperations.first?.presentation().status.text?.contains("lost delivery acknowledgment") == false)
        #expect(try await submissions.receipt(record.id)?.error == older.body?.error)
        #expect(try await submissions.receipt(record.id)?.result == older.body?.result)

        // Retry must release the old read-only observer before looking up the
        // saved command, or its active ownership silently suppresses recovery.
        recovery.retry(record.id)
        _ = await watches.next()
        _ = await watches.next()
        #expect(server.watchCount == 1)
        #expect(server.submissions.isEmpty)
        try await recovery.presented(record.id)
        #expect(server.watchCount == 1)
        #expect(try await submissions.recovery(record.id)?.resultPresented == true)
        #expect(recovery.listedOperations.first?.presentation().actions.contains(.inspect) == true)
        #expect(recovery.listedOperations.first?.presentation().status.text?.contains("planning response lost") == true)
        #expect(try await submissions.pending().first?.payload == record.payload)
    }
}
#endif
