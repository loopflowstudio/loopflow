#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@MainActor
struct OperationRecoveryPresentationTests {
    @Test func currentPlanningStatusReplacesHistoricalFailureWithoutResubmitting() async throws {
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
        #expect(recovery.items.first?.text == "lost delivery acknowledgment")
        _ = await watches.next() // saved-result lookup
        _ = await watches.next() // continued observation after returning the result

        let changes = await submissions.changes()
        server.publish(newer)
        for await _ in changes {
            if try await submissions.receipt(record.id)?.revision == newer.body?.revision { break }
        }
        await recovery.refresh()
        #expect(recovery.items.first?.complete == true)
        #expect(recovery.items.first?.text == "Request completed")
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
        for await _ in changes {
            if await !submissions.isRecovering(record.id) { break }
        }
        #expect(server.watchCount == 0)
        #expect(recovery.items.isEmpty)
    }
}
#endif
