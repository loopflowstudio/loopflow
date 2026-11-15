#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@MainActor
struct OperationRecoveryPresentationTests {
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
        let recovery = CreationRecovery(submissions: submissions)
        var watches = server.observations.makeAsyncIterator()
        _ = await watches.next()
        await recovery.refresh()
        _ = await watches.next()
        _ = await watches.next()
        await recovery.presented([record.id])
        #expect(recovery.items.first?.text.contains("lost delivery acknowledgment") == true)
        #expect(server.watchCount == 1)

        let changes = await submissions.changes()
        server.publish(delivered)
        for await _ in changes {
            if try await submissions.pending().isEmpty { break }
        }
        for await _ in await submissions.changes() {
            if await !submissions.isRecovering(record.id) { break }
        }
        await recovery.refresh()
        #expect(recovery.items.isEmpty)
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
        let recovery = CreationRecovery(submissions: submissions)
        await recovery.refresh()
        #expect(recovery.items.first?.complete == true)
        #expect(recovery.items.first?.text.contains("Planning confirmation unavailable") == false)
        await recovery.presented([record.id])
        for await _ in await submissions.changes() {
            if await !submissions.isRecovering(record.id) { break }
        }
        await recovery.refresh()
        #expect(recovery.items.isEmpty)
        #expect(try await submissions.pending().isEmpty)
        #expect(server.watchCount == 0)
        #expect(server.submissions.isEmpty)
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
        let recovery = CreationRecovery(submissions: submissions)
        await recovery.refresh()
        #expect(recovery.items.first?.complete == false)
        #expect(recovery.items.first?.text.contains("lost delivery acknowledgment") == true)
        _ = await watches.next() // saved-result lookup
        _ = await watches.next() // continued observation after returning the result

        let changes = await submissions.changes()
        server.publish(newer)
        for await _ in changes {
            if try await submissions.receipt(record.id)?.revision == newer.body?.revision { break }
        }
        await recovery.refresh()
        #expect(recovery.items.first?.complete == false)
        #expect(recovery.items.first?.text.contains("planning response lost") == true)
        #expect(recovery.items.first?.text.contains("lost delivery acknowledgment") == false)
        #expect(try await submissions.receipt(record.id)?.error == older.body?.error)
        #expect(try await submissions.receipt(record.id)?.result == older.body?.result)

        // Retry must release the old read-only observer before looking up the
        // saved command, or its active ownership silently suppresses recovery.
        await recovery.retry(record.id)
        _ = await watches.next()
        _ = await watches.next()
        #expect(server.watchCount == 1)
        #expect(server.submissions.isEmpty)
        await recovery.presented([record.id])
        #expect(server.watchCount == 1)
        #expect(try await submissions.recovery(record.id)?.resultPresented == true)
        #expect(recovery.items.first?.complete == false)
        #expect(recovery.items.first?.text.contains("planning response lost") == true)
        #expect(try await submissions.pending().first?.payload == record.payload)
    }
}
#endif
