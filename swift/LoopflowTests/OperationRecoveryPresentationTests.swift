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
            if try await submissions.receipt(record.id)?.revision == delivered.body?.revision { break }
        }
        await recovery.refresh()
        #expect(recovery.items.isEmpty) // only the result prompt was discharged
        #expect(server.watchCount == 1)
        let restarted = client()
        #expect(try await restarted.recovery(record.id)?.resultPresented == true)
        #expect(try await restarted.recovery(record.id)?.planning == .resolved)
        #expect(try await restarted.pending().first?.payload == record.payload)
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
